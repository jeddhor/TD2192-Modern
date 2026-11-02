// SPDX-License-Identifier: MIT
// Copyright (c) 2026 TD2192-Modern contributors
//
// Prepares a Borland Pascal 7 real-mode MZ program that was imported with Ghidra's MZ loader
// (x86:LE:16:Real Mode). Repeatable: running it twice is harmless.
//
//   1. Repairs the loader's off-by-one-page truncation when the MZ header has e_cblp == 0
//      (the last 512-byte page of the image is otherwise left uninitialised).
//   2. Finds DGROUP from the System unit's start-up code (mov reg,imm16 / mov ds,reg), names the
//      DGROUP blocks and marks them non-executable.
//   3. Sets DS = DGROUP as the default register value over every code block.
//   4. Seeds code discovery from the entry point, from every far-call relocation target
//      (9A off seg) and from procedure prologues (55 89 E5, 55 8B EC, C8 nn nn 00) found in
//      undefined bytes, then re-runs auto-analysis. Repeats until nothing new is found.
//   5. Prints coverage statistics.
//
// Arguments (all optional, key:value; key=value also works outside analyzeHeadless.bat):
//   dgroup:<hex>   image-relative DGROUP segment (overrides detection)
//   noanalyze      seed only; do not run auto-analysis
//   passes:<n>     maximum discovery passes (default 4)
//
// Headless: analyzeHeadless <proj> TD2192 -process file0001.exe -noanalysis
//           -scriptPath tools/ghidra -postScript SetupBp7Program.java
//
// @category TD2192
// @author TD2192-Modern

import java.math.BigInteger;
import java.util.*;

import ghidra.app.cmd.disassemble.DisassembleCommand;
import ghidra.app.cmd.function.CreateFunctionCmd;
import ghidra.app.script.GhidraScript;
import ghidra.program.database.mem.FileBytes;
import ghidra.program.model.address.*;
import ghidra.program.model.lang.Register;
import ghidra.program.model.listing.*;
import ghidra.program.model.mem.*;
import ghidra.program.model.reloc.Relocation;
import ghidra.program.model.symbol.*;

public class SetupBp7Program extends GhidraScript {

	private SegmentedAddressSpace space;
	private Memory mem;
	private int imageBase;

	@Override
	public void run() throws Exception {
		Map<String, String> args = parseArgs(getScriptArgs());
		boolean noAnalyze = args.containsKey("noanalyze");
		int passes = Integer.parseInt(args.getOrDefault("passes", "4"));

		space = (SegmentedAddressSpace) currentProgram.getAddressFactory().getDefaultAddressSpace();
		mem = currentProgram.getMemory();
		imageBase = findImageBase();
		println(String.format("[setup] %s: image base segment %04X", currentProgram.getName(),
			imageBase));

		repairTruncatedImage();

		int dgroup;
		if (args.containsKey("dgroup")) {
			dgroup = imageBase + Integer.parseInt(args.get("dgroup"), 16);
		}
		else {
			dgroup = detectDgroup();
		}
		if (dgroup < 0) {
			printerr("[setup] could not detect DGROUP; pass dgroup=<image-relative hex>");
			return;
		}
		println(String.format("[setup] DGROUP = %04X (image-relative %04X)", dgroup,
			dgroup - imageBase));
		labelDgroup(dgroup);
		setDs(dgroup);

		Stats before = stats();
		println("[setup] before: " + before);
		// Ghidra's no-return discovery misfires on BP7 System helpers (string assign etc.) and
		// then drops the code after every call to them. Turn it off and undo its damage.
		setAnalysisOption(currentProgram, "Non-Returning Functions - Discovered", "false");
		for (int pass = 1; pass <= passes; pass++) {
			int wraps = fixNearBranchWrap();
			int cleared = clearBogusTargets();
			int seeded = repairNoReturn() + seedEntry() + seedFarCallTargets() + seedPrologues() +
				seedCodePointers() + seedFlowTargets();
			wraps += fixNearBranchWrap();
			cleared += clearBogusTargets() + clearOrphanText();
			println(String.format(
				"[setup] pass %d: %d rel16 branches re-targeted, %d bogus targets cleared, %d new code locations",
				pass, wraps, cleared, seeded));
			if (!noAnalyze) {
				analyzeAll(currentProgram);
			}
			if (seeded == 0 && wraps == 0 && cleared == 0) {
				break;
			}
		}
		// analysis may have disassembled new rel16 branches: settle without re-analysing
		for (int k = 0; k < 4; k++) {
			int w = fixNearBranchWrap(), c = clearBogusTargets() + clearOrphanText(),
					s = seedFlowTargets();
			if (w + c + s == 0) {
				break;
			}
		}
		fixupFunctionBodies();
		removeFunctionsOutsideCode();
		println("[setup] CS-relative data references re-pointed: " + fixCsRelativeRefs());
		Stats after = stats();
		println("[setup] after:  " + after);
		reportPerBlock();
	}

	// ---------------------------------------------------------------- image base and repair

	private int findImageBase() {
		int best = Integer.MAX_VALUE;
		for (MemoryBlock b : mem.getBlocks()) {
			if (b.getStart() instanceof SegmentedAddress sa && b.isInitialized()) {
				best = Math.min(best, sa.getSegment());
			}
		}
		return best;
	}

	private void repairTruncatedImage() throws Exception {
		List<FileBytes> all = mem.getAllFileBytes();
		if (all.isEmpty()) {
			println("[setup] no FileBytes in program; skipping image-size check");
			return;
		}
		FileBytes fb = all.get(0);
		int cblp = u16(fb, 2), cp = u16(fb, 4), hdrParas = u16(fb, 8);
		long mzEnd = (cblp == 0) ? (long) cp * 512 : (long) (cp - 1) * 512 + cblp;
		mzEnd = Math.min(mzEnd, fb.getSize());
		long hdr = hdrParas * 16L;
		long imgStart = imageBase * 16L;
		long imgEnd = imgStart + (mzEnd - hdr); // exclusive, linear

		for (MemoryBlock b : mem.getBlocks()) {
			if (!(b.getStart() instanceof SegmentedAddress) || b.isInitialized()) {
				continue;
			}
			long bs = b.getStart().getOffset();
			long be = b.getEnd().getOffset() + 1;
			if (bs < imgStart || bs >= imgEnd) {
				continue;
			}
			long fixEnd = Math.min(be, imgEnd);
			println(String.format(
				"[setup] repairing truncated image: %s %s..+%X should hold file bytes (e_cblp=%d)",
				b.getName(), b.getStart(), fixEnd - bs, cblp));
			MemoryBlock target = b;
			if (be > fixEnd) {
				mem.split(b, b.getStart().add(fixEnd - bs));
				target = mem.getBlock(b.getStart());
			}
			Address st = target.getStart();
			long size = target.getSize();
			String name = target.getName();
			mem.removeBlock(target, monitor);
			MemoryBlock nb =
				mem.createInitializedBlock(name, st, fb, hdr + (bs - imgStart), size, false);
			nb.setPermissions(true, true, false);
			MemoryBlock prev = mem.getBlock(st.subtract(1));
			if (prev != null && prev.isInitialized()) {
				mem.join(prev, nb);
			}
			// the remaining uninitialised tail keeps the loader's name with a ".split" suffix
			MemoryBlock tail = mem.getBlock(st.add(size));
			if (tail != null && tail.getName().endsWith(".split")) {
				tail.setName(name);
			}
			return; // only the one truncation case exists
		}
	}

	private static int u16(FileBytes fb, long off) throws java.io.IOException {
		return (fb.getOriginalByte(off) & 0xff) | ((fb.getOriginalByte(off + 1) & 0xff) << 8);
	}

	// ---------------------------------------------------------------- DGROUP

	/** System start-up: entry is "call far System:init"; System init begins with mov reg,DGROUP / mov ds,reg. */
	private int detectDgroup() throws Exception {
		Address entry = entryPoint();
		if (entry == null) {
			return -1;
		}
		Address probe = entry;
		if ((mem.getByte(entry) & 0xff) == 0x9A) {
			int off = mem.getShort(entry.add(1)) & 0xffff;
			int seg = mem.getShort(entry.add(3)) & 0xffff;
			probe = space.getAddress(seg, off);
		}
		byte[] b = new byte[48];
		mem.getBytes(probe, b);
		for (int i = 0; i + 4 < b.length; i++) {
			int op = b[i] & 0xff;
			if (op >= 0xB8 && op <= 0xBB && (b[i + 3] & 0xff) == 0x8E) {
				int reg = op - 0xB8; // ax,cx,dx,bx
				int modrm = b[i + 4] & 0xff;
				if (modrm == (0xD8 | reg)) { // mov ds, reg
					return ((b[i + 1] & 0xff) | ((b[i + 2] & 0xff) << 8));
				}
			}
		}
		// fallback: the segment of the loader's uninitialised DATA block
		for (MemoryBlock blk : mem.getBlocks()) {
			if (!blk.isInitialized() && blk.getStart() instanceof SegmentedAddress sa) {
				return sa.getSegment();
			}
		}
		return -1;
	}

	private void labelDgroup(int dgroup) throws Exception {
		for (MemoryBlock b : mem.getBlocks()) {
			if (!(b.getStart() instanceof SegmentedAddress sa) || sa.getSegment() != dgroup) {
				continue;
			}
			if (b.isInitialized()) {
				if (b.isExecute()) {
					clearListing(b.getStart(), b.getEnd()); // never code
				}
				b.setName("DGROUP");
				b.setPermissions(true, true, false);
			}
			else {
				b.setName("DGROUP_BSS");
			}
		}
		Address a = space.getAddress(dgroup, 0);
		Symbol existing = getSymbolAt(a);
		if (existing != null && "DGROUP".equals(existing.getName())) {
			if (existing.getSource() == SourceType.USER_DEFINED) {
				existing.setSource(SourceType.ANALYSIS); // not a symbols-CSV name
			}
		}
		else {
			createLabel(a, "DGROUP", true, SourceType.ANALYSIS);
		}
		// remove any function the first analysis left in DGROUP
		for (MemoryBlock b : mem.getBlocks()) {
			if (b.getName().startsWith("DGROUP")) {
				for (Function f : currentProgram.getFunctionManager()
						.getFunctions(new AddressSet(b.getStart(), b.getEnd()), true)) {
					removeFunction(f);
				}
			}
		}
	}

	/**
	 * Analysis creates "functions" at the targets of indirect calls through pointer variables
	 * (call far [DS:x]) when it mistakes the variable for the target. Those land in DGROUP or
	 * uninitialised memory; remove them.
	 */
	private void removeFunctionsOutsideCode() {
		List<Function> bad = new ArrayList<>();
		for (Function f : currentProgram.getFunctionManager().getFunctions(true)) {
			if (f.getEntryPoint() instanceof SegmentedAddress && !isCode(f.getEntryPoint())) {
				bad.add(f);
			}
		}
		for (Function f : bad) {
			println("[setup]   removing function outside code: " + f.getEntryPoint());
			removeFunction(f);
		}
	}

	private void setDs(int dgroup) throws Exception {
		ProgramContext ctx = currentProgram.getProgramContext();
		Register ds = ctx.getRegister("DS");
		int n = 0;
		for (MemoryBlock b : mem.getBlocks()) {
			if (b.isExecute() && b.isInitialized() && b.getStart() instanceof SegmentedAddress) {
				ctx.setValue(ds, b.getStart(), b.getEnd(), BigInteger.valueOf(dgroup));
				n++;
			}
		}
		println(String.format("[setup] DS=%04X set over %d code blocks", dgroup, n));
	}

	// ---------------------------------------------------------------- discovery

	private Address entryPoint() {
		var it = currentProgram.getSymbolTable().getExternalEntryPointIterator();
		return it.hasNext() ? it.next() : null;
	}

	private boolean isCode(Address a) {
		MemoryBlock b = mem.getBlock(a);
		return b != null && b.isExecute() && b.isInitialized();
	}

	private boolean isUndefined(Address a) {
		return getInstructionContaining(a) == null && getDataContaining(a) == null;
	}

	private int seed(Address a) {
		if (a == null || !isCode(a)) {
			return 0;
		}
		int n = 0;
		if (getInstructionAt(a) == null) {
			if (!isUndefined(a)) {
				return 0; // inside an existing instruction or data item: leave it to a human
			}
			DisassembleCommand cmd = new DisassembleCommand(a, null, true);
			cmd.applyTo(currentProgram, monitor);
			n++;
		}
		if (getInstructionAt(a) != null && getFunctionAt(a) == null) {
			Function f = createFunction(a, null);
			if (f != null) {
				n++;
			}
		}
		return n;
	}

	private int seedEntry() {
		return seed(entryPoint());
	}

	private int seedFarCallTargets() throws Exception {
		Set<Address> targets = new TreeSet<>();
		Iterator<Relocation> it = currentProgram.getRelocationTable().getRelocations();
		while (it.hasNext()) {
			Relocation r = it.next();
			Address segWord = r.getAddress();
			if (!(segWord instanceof SegmentedAddress) || segWord.getOffset() < 3) {
				continue;
			}
			Address opAddr = segWord.subtract(3);
			if (!isCode(opAddr) || (mem.getByte(opAddr) & 0xff) != 0x9A) {
				continue;
			}
			// must be (or plausibly be) an instruction start
			Instruction ins = getInstructionContaining(opAddr);
			if (ins != null && !ins.getAddress().equals(opAddr)) {
				continue;
			}
			int off = mem.getShort(segWord.subtract(2)) & 0xffff;
			int seg = mem.getShort(segWord) & 0xffff;
			targets.add(space.getAddress(seg, off));
		}
		int n = 0;
		for (Address t : targets) {
			n += seed(t);
		}
		return n;
	}

	private int seedPrologues() throws Exception {
		int n = 0;
		for (MemoryBlock b : mem.getBlocks()) {
			if (!b.isExecute() || !b.isInitialized() || !(b.getStart() instanceof SegmentedAddress)) {
				continue;
			}
			byte[] bytes = new byte[(int) b.getSize()];
			b.getBytes(b.getStart(), bytes);
			for (int i = 0; i + 4 <= bytes.length; i++) {
				int b0 = bytes[i] & 0xff, b1 = bytes[i + 1] & 0xff, b2 = bytes[i + 2] & 0xff;
				boolean prologue = (b0 == 0x55 && b1 == 0x89 && b2 == 0xE5) ||
					(b0 == 0x55 && b1 == 0x8B && b2 == 0xEC) ||
					(b0 == 0xC8 && (bytes[i + 3] & 0xff) == 0x00 && (b1 & 1) == 0);
				if (!prologue) {
					continue;
				}
				Address a = b.getStart().add(i);
				if (isUndefined(a)) {
					n += seed(a);
				}
			}
		}
		return n;
	}

	// ---------------------------------------------------------------- rel16 wrap repair

	/** Wrong targets produced by the SLEIGH rel16 computation, pending clean-up. */
	private final Set<Address> wrongTargets = new TreeSet<>();

	/**
	 * Ghidra's x86 SLEIGH computes a rel16 target as
	 * {@code ((inst_next >> 16) << 16) | ((inst_next + rel) & 0xFFFF)} on the *linear* address,
	 * i.e. it wraps at a 64 KB linear boundary instead of at the segment base. In any segment
	 * that straddles a 64 KB linear boundary, every near CALL/JMP rel16 that crosses that
	 * boundary therefore lands 64 KB off. The real-mode target is
	 * {@code CS:((IP_next + rel) & 0xFFFF)}. This replaces the wrong flow reference with an
	 * overriding reference (honoured by flow following and by the decompiler).
	 */
	private int fixNearBranchWrap() throws Exception {
		ReferenceManager rm = currentProgram.getReferenceManager();
		int fixed = 0;
		for (MemoryBlock b : mem.getBlocks()) {
			if (!b.isExecute() || !b.isInitialized() || !(b.getStart() instanceof SegmentedAddress sa)) {
				continue;
			}
			long s = b.getStart().getOffset(), e = b.getEnd().getOffset();
			if ((s >> 16) == (e >> 16)) {
				continue; // does not straddle a 64 KB linear boundary
			}
			int seg = sa.getSegment();
			InstructionIterator it = currentProgram.getListing()
					.getInstructions(new AddressSet(b.getStart(), b.getEnd()), true);
			while (it.hasNext()) {
				Instruction ins = it.next();
				int op = mem.getByte(ins.getAddress()) & 0xff;
				if ((op != 0xE8 && op != 0xE9) || ins.getLength() != 3) {
					continue;
				}
				int rel = mem.getShort(ins.getAddress().add(1)); // signed
				long ipNext = ins.getAddress().getOffset() - seg * 16L + 3;
				long correctLinear = seg * 16L + ((ipNext + rel) & 0xFFFF);
				Address[] dflt = ins.getDefaultFlows();
				if (dflt.length == 1 && dflt[0].getOffset() == correctLinear) {
					continue; // Ghidra got this one right
				}
				Address correct = space.getAddress(seg, (int) (correctLinear - seg * 16L));
				boolean haveCorrect = false;
				for (Reference r : rm.getFlowReferencesFrom(ins.getAddress())) {
					if (r.getToAddress().getOffset() == correctLinear) {
						haveCorrect = true;
					}
					else {
						wrongTargets.add(r.getToAddress());
						rm.delete(r);
					}
				}
				if (!haveCorrect) {
					RefType t = op == 0xE8 ? RefType.CALL_OVERRIDE_UNCONDITIONAL
							: RefType.JUMP_OVERRIDE_UNCONDITIONAL;
					Reference nr = rm.addMemoryReference(ins.getAddress(), correct, t,
						SourceType.USER_DEFINED, 0);
					rm.setPrimary(nr, true);
					fixed++;
				}
			}
		}
		return fixed;
	}

	/**
	 * In real mode the SLEIGH derives CS as {@code (linear >> 4) & 0xF000} (a 64 KB-normalised
	 * segment), so a "CS:[disp]" operand references the wrong place unless the segment is
	 * 64 KB aligned. Re-points each such memory reference to {@code <own segment>:disp}.
	 * (The decompiler still prints the raw constant; read CS:[x] as own-segment:x.)
	 */
	private int fixCsRelativeRefs() throws Exception {
		ReferenceManager rm = currentProgram.getReferenceManager();
		int fixed = 0;
		for (MemoryBlock b : mem.getBlocks()) {
			if (!b.isExecute() || !b.isInitialized() || !(b.getStart() instanceof SegmentedAddress sa)) {
				continue;
			}
			int seg = sa.getSegment();
			InstructionIterator it = currentProgram.getListing()
					.getInstructions(new AddressSet(b.getStart(), b.getEnd()), true);
			while (it.hasNext()) {
				Instruction ins = it.next();
				long normSegBase = ((ins.getAddress().getOffset() >> 4) & 0xF000) * 16L;
				if (normSegBase == seg * 16L) {
					continue;
				}
				for (int op = 0; op < ins.getNumOperands(); op++) {
					String text = ins.getDefaultOperandRepresentation(op).toUpperCase();
					if (!text.contains("CS:[")) {
						continue;
					}
					for (Reference r : ins.getOperandReferences(op)) {
						if (!r.isMemoryReference() || r.getReferenceType().isFlow()) {
							continue;
						}
						long disp = r.getToAddress().getOffset() - normSegBase;
						if (disp < 0 || disp > 0xFFFF) {
							continue; // already re-pointed, or not of this shape
						}
						Address correct = space.getAddress(seg, (int) disp);
						rm.delete(r);
						Reference nr = rm.addMemoryReference(ins.getAddress(), correct,
							r.getReferenceType(), SourceType.USER_DEFINED, op);
						rm.setPrimary(nr, true);
						fixed++;
					}
				}
			}
		}
		return fixed;
	}

	/**
	 * Removes functions and orphan instruction runs that exist only because of a wrong rel16
	 * target (no flow reference reaches them any more). Real code inside a removed body is
	 * rediscovered by the seeding passes from the corrected references.
	 */
	private int clearBogusTargets() throws Exception {
		int n = 0;
		ReferenceManager rm = currentProgram.getReferenceManager();
		for (Address w : new ArrayList<>(wrongTargets)) {
			if (hasFlowRefTo(rm, w)) {
				continue;
			}
			wrongTargets.remove(w);
			Function f = getFunctionAt(w);
			if (f != null) {
				AddressSetView body = new AddressSet(f.getBody());
				removeFunction(f);
				for (AddressRange r : body) {
					clearListing(r.getMinAddress(), r.getMaxAddress());
				}
				n++;
				continue;
			}
			// orphan run reached by a wrong JMP: clear while nothing else flows into it
			Address a = w;
			int guard = 0;
			while (a != null && guard++ < 4096) {
				Instruction ins = getInstructionAt(a);
				if (ins == null || getFunctionContaining(a) != null) {
					break;
				}
				if (!a.equals(w) && hasFlowRefTo(rm, a)) {
					break;
				}
				Address next = ins.getFallThrough();
				clearListing(ins.getAddress(), ins.getMaxAddress());
				a = next;
			}
			n++;
		}
		return n;
	}

	/**
	 * Clears runs of instructions that belong to no function, are not the target of any flow
	 * reference from code inside a function, and whose bytes are at least 70% printable ASCII:
	 * that is string data decoded as code (lowercase text decodes as INS/OUTS/IMUL/JO...).
	 */
	private int clearOrphanText() throws Exception {
		int n = 0;
		ReferenceManager rm = currentProgram.getReferenceManager();
		for (MemoryBlock b : mem.getBlocks()) {
			if (!b.isExecute() || !b.isInitialized()) {
				continue;
			}
			List<Instruction> run = new ArrayList<>();
			InstructionIterator it = currentProgram.getListing()
					.getInstructions(new AddressSet(b.getStart(), b.getEnd()), true);
			while (true) {
				Instruction ins = it.hasNext() ? it.next() : null;
				boolean orphan = ins != null && getFunctionContaining(ins.getAddress()) == null;
				boolean contiguous = orphan && (run.isEmpty() ||
					run.get(run.size() - 1).getMaxAddress().add(1).equals(ins.getAddress()));
				if (contiguous) {
					run.add(ins);
					continue;
				}
				if (!run.isEmpty()) {
					n += clearIfText(rm, run);
					run.clear();
				}
				if (ins == null) {
					break;
				}
				if (orphan) {
					run.add(ins);
				}
			}
		}
		return n;
	}

	private int clearIfText(ReferenceManager rm, List<Instruction> run) throws Exception {
		Address start = run.get(0).getAddress();
		Address end = run.get(run.size() - 1).getMaxAddress();
		long len = end.getOffset() - start.getOffset() + 1;
		if (len < 8) {
			return 0;
		}
		for (Instruction ins : run) {
			for (Reference r : rm.getReferencesTo(ins.getAddress())) {
				if (r.getReferenceType().isFlow() &&
					getFunctionContaining(r.getFromAddress()) != null) {
					return 0; // reached from real code
				}
			}
		}
		byte[] d = new byte[(int) len];
		mem.getBytes(start, d);
		int printable = 0;
		for (byte c : d) {
			int u = c & 0xff;
			if (u >= 0x20 && u < 0x7f) {
				printable++;
			}
		}
		if (printable * 10 < len * 7) {
			return 0;
		}
		clearListing(start, end);
		return 1;
	}

	private static boolean hasFlowRefTo(ReferenceManager rm, Address a) {
		for (Reference r : rm.getReferencesTo(a)) {
			if (r.getReferenceType().isFlow()) {
				return true;
			}
		}
		return false;
	}

	/** Disassembles every undefined target of an existing call or jump reference. */
	private int seedFlowTargets() throws Exception {
		Set<Address> calls = new TreeSet<>(), jumps = new TreeSet<>();
		ReferenceManager rm = currentProgram.getReferenceManager();
		for (MemoryBlock b : mem.getBlocks()) {
			if (!b.isExecute() || !b.isInitialized()) {
				continue;
			}
			InstructionIterator it = currentProgram.getListing()
					.getInstructions(new AddressSet(b.getStart(), b.getEnd()), true);
			while (it.hasNext()) {
				Instruction ins = it.next();
				for (Reference r : rm.getFlowReferencesFrom(ins.getAddress())) {
					Address t = r.getToAddress();
					if (isCode(t) && isUndefined(t)) {
						(r.getReferenceType().isCall() ? calls : jumps).add(t);
					}
				}
			}
		}
		int n = 0;
		for (Address t : jumps) {
			if (isUndefined(t)) {
				new DisassembleCommand(t, null, true).applyTo(currentProgram, monitor);
				n++;
			}
		}
		for (Address t : calls) {
			n += seed(t);
		}
		return n;
	}

	private void fixupFunctionBodies() throws Exception {
		for (Function f : currentProgram.getFunctionManager().getFunctions(true)) {
			if (isCode(f.getEntryPoint())) {
				if (getInstructionAt(f.getEntryPoint()) == null && isUndefined(f.getEntryPoint())) {
					new DisassembleCommand(f.getEntryPoint(), null, true).applyTo(currentProgram,
						monitor);
				}
				CreateFunctionCmd.fixupFunctionBody(currentProgram, f, monitor);
			}
		}
	}

	/**
	 * Clears the no-return flag on every function whose body contains a return instruction
	 * (RET/RETF/IRET), then re-disassembles each call site whose fall-through was dropped.
	 * Functions without any return (System Halt/RunError paths) keep the flag.
	 */
	private int repairNoReturn() throws Exception {
		int n = 0;
		List<Function> fixed = new ArrayList<>();
		for (Function f : currentProgram.getFunctionManager().getFunctions(true)) {
			if (!f.hasNoReturn()) {
				continue;
			}
			boolean hasReturn = false;
			InstructionIterator it = currentProgram.getListing().getInstructions(f.getBody(), true);
			while (it.hasNext() && !hasReturn) {
				String m = it.next().getMnemonicString().toUpperCase();
				hasReturn = m.startsWith("RET") || m.startsWith("IRET");
			}
			if (hasReturn) {
				f.setNoReturn(false);
				fixed.add(f);
			}
			else {
				println("[setup]   keeps no-return: " + f.getName() + " @ " + f.getEntryPoint());
			}
		}
		for (Function f : fixed) {
			println("[setup]   cleared no-return: " + f.getName() + " @ " + f.getEntryPoint());
			for (var ref : getReferencesTo(f.getEntryPoint())) {
				if (!ref.getReferenceType().isCall()) {
					continue;
				}
				Instruction call = getInstructionAt(ref.getFromAddress());
				if (call == null) {
					continue;
				}
				Address ft = call.getAddress().add(call.getLength());
				if (isCode(ft) && isUndefined(ft)) {
					Address ca = call.getAddress();
					clearListing(ca, ca.add(call.getLength() - 1));
					new DisassembleCommand(ca, null, true).applyTo(currentProgram, monitor);
					n++;
				}
			}
		}
		return n;
	}

	/**
	 * Code reached only through pointers (interrupt handlers, ExitProc, TextRec driver
	 * functions, procedure variables): an immediate operand next to PUSH CS, or next to an
	 * instruction whose operand is a relocated code-segment word, names an offset in that
	 * segment. Targets that look like Pascal string literals are skipped.
	 */
	private int seedCodePointers() throws Exception {
		Set<Address> cands = new TreeSet<>();
		Listing listing = currentProgram.getListing();
		for (MemoryBlock b : mem.getBlocks()) {
			if (!b.isExecute() || !b.isInitialized() || !(b.getStart() instanceof SegmentedAddress sa)) {
				continue;
			}
			InstructionIterator it = listing.getInstructions(new AddressSet(b.getStart(), b.getEnd()), true);
			while (it.hasNext()) {
				Instruction ins = it.next();
				if (ins.getMnemonicString().equalsIgnoreCase("PUSH") && ins.getNumOperands() == 1 &&
					"CS".equalsIgnoreCase(ins.getDefaultOperandRepresentation(0))) {
					collectNeighbourImmediates(ins, sa.getSegment(), null, cands);
				}
			}
		}
		Iterator<Relocation> rit = currentProgram.getRelocationTable().getRelocations();
		while (rit.hasNext()) {
			Address segWord = rit.next().getAddress();
			if (!isCode(segWord) || segWord.getOffset() < 3 ||
				(mem.getByte(segWord.subtract(3)) & 0xff) == 0x9A) {
				continue;
			}
			int seg = mem.getShort(segWord) & 0xffff;
			Address segStart = space.getAddress(seg, 0);
			if (!isCode(segStart)) {
				continue;
			}
			Instruction ins = getInstructionContaining(segWord);
			if (ins != null) {
				collectNeighbourImmediates(ins, seg, ins, cands);
			}
		}
		int n = 0;
		for (Address c : cands) {
			if (isCode(c) && isUndefined(c) && !looksLikeDataStart(c)) {
				n += seed(c);
			}
		}
		return n;
	}

	private void collectNeighbourImmediates(Instruction center, int seg, Instruction exclude,
			Set<Address> out) {
		List<Instruction> near = new ArrayList<>();
		Instruction p = center, q = center;
		near.add(center);
		for (int k = 0; k < 3; k++) {
			p = p == null ? null : p.getPrevious();
			q = q == null ? null : q.getNext();
			if (p != null) {
				near.add(p);
			}
			if (q != null) {
				near.add(q);
			}
		}
		MemoryBlock blk = mem.getBlock(space.getAddress(seg, 0));
		for (Instruction ins : near) {
			if (ins == exclude) {
				continue;
			}
			for (int i = 0; i < ins.getNumOperands(); i++) {
				int t = ins.getOperandType(i);
				if ((t & ghidra.program.model.lang.OperandType.SCALAR) == 0 ||
					(t & ghidra.program.model.lang.OperandType.DYNAMIC) != 0) {
					continue;
				}
				ghidra.program.model.scalar.Scalar s = ins.getScalar(i);
				if (s == null) {
					continue;
				}
				long v = s.getUnsignedValue() & 0xffff;
				if (blk != null && v < blk.getSize()) {
					out.add(space.getAddress(seg, (int) v));
				}
			}
		}
	}

	/** True for a plausible Pascal string literal or zero fill at {@code a}. */
	private boolean looksLikeDataStart(Address a) throws Exception {
		int len = mem.getByte(a) & 0xff;
		if (len == 0) {
			return true;
		}
		byte[] s = new byte[len];
		try {
			mem.getBytes(a.add(1), s);
		}
		catch (Exception e) {
			return false;
		}
		int printable = 0;
		for (byte c : s) {
			int u = c & 0xff;
			if (u == 0) {
				return false;
			}
			if (u >= 0x20 && u < 0x7f) {
				printable++;
			}
		}
		return printable * 4 >= len * 3;
	}

	// ---------------------------------------------------------------- statistics

	private record Stats(int functions, long codeBytes, long instrBytes, long strBytes,
			long zeroBytes, long otherBytes) {
		@Override
		public String toString() {
			return String.format(
				"%d functions; %d of %d code-block bytes are instructions (%.1f%%); " +
					"not code: %d in Pascal strings, %d zero, %d other (%.1f%% of code blocks)",
				functions, instrBytes, codeBytes, pct(instrBytes, codeBytes), strBytes, zeroBytes,
				otherBytes, pct(otherBytes, codeBytes));
		}

		private static double pct(long a, long b) {
			return b == 0 ? 0.0 : 100.0 * a / b;
		}
	}

	private Stats stats() throws Exception {
		long code = 0, instr = 0;
		long[] cls = new long[3];
		Listing listing = currentProgram.getListing();
		for (MemoryBlock b : mem.getBlocks()) {
			if (b.isExecute() && b.isInitialized() && b.getStart() instanceof SegmentedAddress) {
				AddressSet set = new AddressSet(b.getStart(), b.getEnd());
				code += b.getSize();
				InstructionIterator ii = listing.getInstructions(set, true);
				while (ii.hasNext()) {
					instr += ii.next().getLength();
				}
				for (AddressRange r : listing.getUndefinedRanges(set, false, monitor)) {
					byte[] d = new byte[(int) r.getLength()];
					mem.getBytes(r.getMinAddress(), d);
					classifyGap(d, cls);
				}
			}
		}
		return new Stats(currentProgram.getFunctionManager().getFunctionCount(), code, instr,
			cls[0], cls[1], cls[2]);
	}

	/** Splits an undefined run into Pascal-string bytes, zero bytes and other bytes. */
	private static void classifyGap(byte[] d, long[] cls) {
		int i = 0;
		while (i < d.length) {
			int len = d[i] & 0xff;
			boolean str = len >= 1 && i + len < d.length;
			for (int k = 1; str && k <= len; k++) {
				int c = d[i + k] & 0xff;
				str = c >= 0x20 && c < 0x7f;
			}
			if (str) {
				cls[0] += len + 1;
				i += len + 1;
				continue;
			}
			cls[d[i] == 0 ? 1 : 2]++;
			i++;
		}
	}

	private void reportPerBlock() {
		for (MemoryBlock b : mem.getBlocks()) {
			if (!(b.getStart() instanceof SegmentedAddress sa)) {
				continue;
			}
			AddressSet set = new AddressSet(b.getStart(), b.getEnd());
			long instr = 0;
			InstructionIterator ii = currentProgram.getListing().getInstructions(set, true);
			while (ii.hasNext()) {
				instr += ii.next().getLength();
			}
			int funcs = 0;
			for (Function f : currentProgram.getFunctionManager().getFunctions(set, true)) {
				funcs++;
			}
			println(String.format("[setup]   %-12s rel %04X size %6X init=%-5s x=%-5s funcs %4d instr %6d (%.0f%%)",
				b.getName(), sa.getSegment() - imageBase, b.getSize(), b.isInitialized(),
				b.isExecute(), funcs, instr, b.getSize() == 0 ? 0.0 : 100.0 * instr / b.getSize()));
		}
	}

	// ---------------------------------------------------------------- helpers

	private static Map<String, String> parseArgs(String[] a) {
		Map<String, String> m = new HashMap<>();
		for (String s : a) {
			int eq = s.indexOf('=') >= 0 ? s.indexOf('=') : s.indexOf(':');
			if (eq < 0) {
				m.put(s.toLowerCase(), "");
			}
			else {
				m.put(s.substring(0, eq).toLowerCase(), s.substring(eq + 1));
			}
		}
		return m;
	}
}
