// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (c) 2026 TD2192-Modern contributors
//
// Writes a plain disassembly listing per function: <outdir>/<SSSS>_<OOOO>_<name>.asm. Every
// line starts with the image-relative address (ADG 7.8), then the instruction bytes and text.
// Flow and data references are appended as image-relative addresses (with the target's name),
// so near calls that Ghidra's SLEIGH resolves 64 KB off (EXECUTABLES.md 1.1, defect 5) show
// the override target that SetupBp7Program installed. Read "CS:[x]" as own-segment:x.
//
// The listing is derived from the original program. It is written ONLY outside any git work
// tree (default: <project parent>/exports/<exe>-asm/). Never commit it, and never paste or
// transliterate it into the port (AGENTS.md rules 3 and 4).
//
// Arguments: [outdir] [filter:<regex>] [range:SSSS:OOOO-SSSS:OOOO]
//   filter is searched for in "SSSS:OOOO name" of each function (e.g. filter:^051A:).
//   range dumps the instructions in an address range into range_<start>.asm instead (it may be
//   repeated). key=value also works from the GUI.
//
// @category TD2192
// @author TD2192-Modern

import java.io.*;
import java.util.*;
import java.util.regex.Pattern;

import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.*;
import ghidra.program.model.listing.*;
import ghidra.program.model.mem.MemoryBlock;
import ghidra.program.model.symbol.*;

public class ExportListing extends GhidraScript {

	private int imageBase;

	@Override
	public void run() throws Exception {
		imageBase = findImageBase();
		String stem = currentProgram.getName().replaceAll("\\.[^.]*$", "");
		File outDir = null;
		Pattern filter = null;
		List<String> ranges = new ArrayList<>();
		for (String a : getScriptArgs()) {
			if (opt(a, "filter") != null) {
				filter = Pattern.compile(opt(a, "filter"));
			}
			else if (opt(a, "range") != null) {
				ranges.add(opt(a, "range"));
			}
			else {
				outDir = new File(a);
			}
		}
		if (outDir == null) {
			outDir = defaultExport(stem + "-asm");
		}
		refuseInsideGitTree(outDir);
		outDir.mkdirs();

		int n = 0;
		if (!ranges.isEmpty()) {
			for (String r : ranges) {
				String[] p = r.split("-");
				Address s = relAddr(p[0]);
				Address e = relAddr(p[1]);
				File f = new File(outDir, "range_" + p[0].replace(':', '_') + ".asm");
				try (PrintWriter pw = new PrintWriter(new OutputStreamWriter(new FileOutputStream(f), "UTF-8"))) {
					pw.printf("; %s range %s (listing derived from the original: do not commit)%n", stem, r);
					dump(pw, new AddressSet(s, e));
				}
				n++;
			}
		}
		else {
			for (Function fn : currentProgram.getFunctionManager().getFunctions(true)) {
				monitor.checkCancelled();
				if (!(fn.getEntryPoint() instanceof SegmentedAddress)) {
					continue;
				}
				String r = rel(fn.getEntryPoint());
				if (filter != null && !filter.matcher(r + " " + fn.getName()).find()) {
					continue;
				}
				String fileName = r.replace(':', '_') + "_" + fn.getName().replaceAll("[^A-Za-z0-9_]", "_") + ".asm";
				try (PrintWriter pw = new PrintWriter(new OutputStreamWriter(
					new FileOutputStream(new File(outDir, fileName)), "UTF-8"))) {
					pw.printf("; %s:%s %s (listing derived from the original: do not commit)%n", stem, r, fn.getName());
					dump(pw, fn.getBody());
				}
				n++;
			}
		}
		println(String.format("[listing] %d files written to %s", n, outDir));
	}

	/** Image-relative "SSSS:OOOO" to a Ghidra address. */
	private Address relAddr(String relAddr) {
		String[] p = relAddr.split(":");
		int seg = Integer.parseInt(p[0], 16) + imageBase;
		int off = Integer.parseInt(p[1], 16);
		return ((SegmentedAddressSpace) currentProgram.getAddressFactory().getDefaultAddressSpace())
				.getAddress(seg, off);
	}

	private void dump(PrintWriter pw, AddressSetView set) throws Exception {
		Listing listing = currentProgram.getListing();
		ReferenceManager rm = currentProgram.getReferenceManager();
		SymbolTable st = currentProgram.getSymbolTable();
		for (Instruction ins : listing.getInstructions(set, true)) {
			StringBuilder bytes = new StringBuilder();
			for (byte b : ins.getBytes()) {
				bytes.append(String.format("%02X", b & 0xFF));
			}
			Symbol lbl = st.getPrimarySymbol(ins.getAddress());
			if (lbl != null && lbl.getSource() != SourceType.DEFAULT) {
				pw.printf("%s:%n", lbl.getName());
			}
			StringBuilder refs = new StringBuilder();
			for (Reference ref : rm.getReferencesFrom(ins.getAddress())) {
				Address to = ref.getToAddress();
				if (!(to instanceof SegmentedAddress)) {
					continue;
				}
				Symbol s = st.getPrimarySymbol(to);
				refs.append(' ').append(ref.getReferenceType().isFlow() ? "->" : "@").append(rel(to));
				if (s != null) {
					refs.append('(').append(s.getName()).append(')');
				}
			}
			pw.printf("%s  %-14s %s%s%n", rel(ins.getAddress()), bytes, ins.toString(),
				refs.length() > 0 ? "   ;" + refs : "");
		}
	}

	// ---------------------------------------------------------------- helpers (shared shape)

	/** Value of a "key:value" or "key=value" argument, else null. */
	private static String opt(String a, String key) {
		return a.startsWith(key + ":") || a.startsWith(key + "=") ? a.substring(key.length() + 1) : null;
	}

	private void refuseInsideGitTree(File out) {
		for (File d = out.getAbsoluteFile(); d != null; d = d.getParentFile()) {
			if (new File(d, ".git").exists()) {
				throw new IllegalArgumentException("refusing to write a listing inside a git work tree: " + d);
			}
		}
	}

	private int findImageBase() {
		int best = Integer.MAX_VALUE;
		for (MemoryBlock b : currentProgram.getMemory().getBlocks()) {
			if (b.getStart() instanceof SegmentedAddress sa && b.isInitialized()) {
				best = Math.min(best, sa.getSegment());
			}
		}
		return best;
	}

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
}
