// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (c) 2026 TD2192-Modern contributors
//
// Writes a CSV index of every function in the current program:
//   address (image-relative SSSS:OOOO), ghidra_address, name, segment (image-relative),
//   size (body bytes), ret (near/far/iret + bytes popped), prologue (bp89 | enter | bp8b | none),
//   has_stack_check, callers (distinct calling functions), callees (distinct called functions)
//
// The BP7 stack-check routine is recognised as the most common target of the
// "mov ax,<frame size>; call far X" pair at procedure entry (or by the name BP_StackCheck).
//
// Arguments: [output.csv]   default: <project parent>/exports/<exe>-functions.csv
//
// @category TD2192
// @author TD2192-Modern

import java.io.*;
import java.util.*;

import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.*;
import ghidra.program.model.listing.*;
import ghidra.program.model.mem.MemoryBlock;

public class ExportFunctionIndex extends GhidraScript {

	private int imageBase;

	@Override
	public void run() throws Exception {
		imageBase = findImageBase();
		File out = getScriptArgs().length > 0 ? new File(getScriptArgs()[0])
				: defaultExport(currentProgram.getName().replaceAll("\\.[^.]*$", "") + "-functions.csv");
		out.getAbsoluteFile().getParentFile().mkdirs();

		FunctionManager fm = currentProgram.getFunctionManager();
		Map<Address, Integer> stackCheckVotes = new HashMap<>();
		for (Function f : fm.getFunctions(true)) {
			Address t = entryFarCallAfterMovAx(f);
			if (t != null) {
				stackCheckVotes.merge(t, 1, Integer::sum);
			}
		}
		Address stackCheck = null;
		for (Function f : fm.getFunctions(true)) {
			if (f.getName().equals("BP_StackCheck")) {
				stackCheck = f.getEntryPoint();
			}
		}
		if (stackCheck == null) {
			stackCheck = stackCheckVotes.entrySet().stream().max(Map.Entry.comparingByValue())
					.map(Map.Entry::getKey).orElse(null);
		}
		println("[index] stack-check routine: " + (stackCheck == null ? "none" : rel(stackCheck)));

		int n = 0;
		try (PrintWriter pw = new PrintWriter(new OutputStreamWriter(new FileOutputStream(out), "UTF-8"))) {
			pw.println("address,ghidra_address,name,segment,size,ret,prologue,has_stack_check,callers,callees");
			for (Function f : fm.getFunctions(true)) {
				if (!(f.getEntryPoint() instanceof SegmentedAddress)) {
					continue;
				}
				Address e = f.getEntryPoint();
				String r = rel(e);
				Set<Function> callers = f.getCallingFunctions(monitor);
				Set<Function> callees = f.getCalledFunctions(monitor);
				boolean sc = stackCheck != null && stackCheck.equals(entryFarCallAfterMovAx(f));
				pw.println(String.join(",", r, e.toString(), csv(f.getName()), r.substring(0, 4),
					Long.toString(f.getBody().getNumAddresses()), retKind(f), prologue(f),
					sc ? "yes" : "no", Integer.toString(callers.size()),
					Integer.toString(callees.size())));
				n++;
			}
		}
		println("[index] wrote " + n + " functions to " + out);
	}

	/** Target of a far call that directly follows "mov ax,imm" within the first 4 instructions. */
	private Address entryFarCallAfterMovAx(Function f) {
		Instruction ins = getInstructionAt(f.getEntryPoint());
		Instruction prev = null;
		for (int i = 0; i < 4 && ins != null; i++) {
			if (prev != null && prev.getMnemonicString().equalsIgnoreCase("MOV") &&
				"AX".equalsIgnoreCase(prev.getDefaultOperandRepresentation(0)) &&
				ins.getMnemonicString().equalsIgnoreCase("CALLF")) {
				Address[] flows = ins.getFlows();
				return flows.length == 1 ? flows[0] : null;
			}
			prev = ins;
			ins = ins.getNext();
		}
		return null;
	}

	private String prologue(Function f) {
		byte[] b = new byte[4];
		try {
			currentProgram.getMemory().getBytes(f.getEntryPoint(), b);
		}
		catch (Exception e) {
			return "unreadable";
		}
		int b0 = b[0] & 0xff, b1 = b[1] & 0xff, b2 = b[2] & 0xff;
		if (b0 == 0x55 && b1 == 0x89 && b2 == 0xE5) {
			return "bp89";
		}
		if (b0 == 0x55 && b1 == 0x8B && b2 == 0xEC) {
			return "bp8b";
		}
		if (b0 == 0xC8 && (b[3] & 0xff) == 0) {
			return "enter";
		}
		return "none";
	}

	private String retKind(Function f) {
		InstructionIterator it = currentProgram.getListing().getInstructions(f.getBody(), true);
		String kind = "";
		while (it.hasNext()) {
			Instruction ins = it.next();
			String m = ins.getMnemonicString().toUpperCase();
			String k = null;
			if (m.equals("RETF")) {
				k = "far";
			}
			else if (m.equals("RET")) {
				k = "near";
			}
			else if (m.startsWith("IRET")) {
				k = "iret";
			}
			if (k != null) {
				String pop = ins.getNumOperands() > 0 ? " " + ins.getDefaultOperandRepresentation(0) : "";
				String s = k + pop;
				if (kind.isEmpty()) {
					kind = s;
				}
				else if (!kind.contains(s)) {
					kind = kind + "|" + s;
				}
			}
		}
		return kind.isEmpty() ? "none" : csv(kind);
	}

	// ---------------------------------------------------------------- helpers (shared shape)

	private int findImageBase() {
		int best = Integer.MAX_VALUE;
		for (MemoryBlock b : currentProgram.getMemory().getBlocks()) {
			if (b.getStart() instanceof SegmentedAddress sa && b.isInitialized()) {
				best = Math.min(best, sa.getSegment());
			}
		}
		return best;
	}

	/** Image-relative SSSS:OOOO, using the segment of the containing memory block. */
	private String rel(Address a) {
		MemoryBlock b = currentProgram.getMemory().getBlock(a);
		int seg = (b != null && b.getStart() instanceof SegmentedAddress sa) ? sa.getSegment()
				: (int) (a.getOffset() >> 4);
		long off = a.getOffset() - seg * 16L;
		if (off < 0 || off > 0xFFFF) {
			seg = (int) (a.getOffset() >> 4);
			off = a.getOffset() & 0xF;
		}
		return String.format("%04X:%04X", (seg - imageBase) & 0xFFFF, off);
	}

	private File defaultExport(String name) {
		File proj = state.getProject().getProjectLocator().getProjectDir().getParentFile();
		return new File(new File(proj.getParentFile(), "exports"), name);
	}

	private static String csv(String s) {
		if (s.contains(",") || s.contains("\"") || s.contains("\n")) {
			return "\"" + s.replace("\"", "\"\"") + "\"";
		}
		return s;
	}
}
