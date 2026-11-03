// SPDX-License-Identifier: MIT
// Copyright (c) 2026 TD2192-Modern contributors
//
// Writes one TSV row per instruction of the current program, for cross-episode matching (M1-09,
// tools/scripts/match-episodes.py). Columns:
//   func      image-relative entry of the containing function, or "-" if none
//   addr      image-relative SSSS:OOOO of the instruction (ADG 7.8)
//   bytes     instruction bytes (hex)
//   mask      Ghidra's instruction mask (hex, same length): 1-bits are opcode bits, 0-bits are
//             operand fields (displacements, immediates, register fields that vary)
//   mnemonic  mnemonic only
//   refs      references from the instruction, ";"-separated: F<SSSS:OOOO> flow (calls and
//             jumps, including SetupBp7Program's overrides), D<SSSS:OOOO> data
// A second file <stem>-funcs.tsv lists every function: entry, name, body bytes, instructions.
//
// The table is derived from the original program (it contains its instruction bytes). It is
// written ONLY outside any git work tree (default: <project parent>/exports/<exe>-insns.tsv).
// Never commit it (AGENTS.md rules 2 and 3).
//
// Arguments: [outdir]
//
// @category TD2192
// @author TD2192-Modern

import java.io.*;
import java.util.*;

import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.*;
import ghidra.program.model.lang.InstructionPrototype;
import ghidra.program.model.listing.*;
import ghidra.program.model.mem.MemoryBlock;
import ghidra.program.model.symbol.*;

public class ExportInstructionTable extends GhidraScript {

	private int imageBase;

	@Override
	public void run() throws Exception {
		imageBase = findImageBase();
		String stem = currentProgram.getName().replaceAll("\\.[^.]*$", "");
		File outDir = getScriptArgs().length > 0 ? new File(getScriptArgs()[0]) : defaultExport("");
		refuseInsideGitTree(outDir);
		outDir.mkdirs();

		Listing listing = currentProgram.getListing();
		FunctionManager fm = currentProgram.getFunctionManager();
		ReferenceManager rm = currentProgram.getReferenceManager();
		int n = 0;
		try (PrintWriter pw = new PrintWriter(new OutputStreamWriter(
			new FileOutputStream(new File(outDir, stem + "-insns.tsv")), "UTF-8"))) {
			pw.println("func\taddr\tbytes\tmask\tmnemonic\trefs");
			for (Instruction ins : listing.getInstructions(true)) {
				monitor.checkCancelled();
				if (!(ins.getAddress() instanceof SegmentedAddress)) {
					continue;
				}
				Function f = fm.getFunctionContaining(ins.getAddress());
				byte[] b = ins.getBytes();
				byte[] m = maskOf(ins, b.length);
				StringBuilder refs = new StringBuilder();
				for (Reference ref : rm.getReferencesFrom(ins.getAddress())) {
					Address to = ref.getToAddress();
					if (!(to instanceof SegmentedAddress)) {
						continue;
					}
					if (refs.length() > 0) {
						refs.append(';');
					}
					refs.append(ref.getReferenceType().isFlow() ? 'F' : 'D').append(rel(to));
				}
				pw.printf("%s\t%s\t%s\t%s\t%s\t%s%n", f == null ? "-" : rel(f.getEntryPoint()),
					rel(ins.getAddress()), hex(b), hex(m), ins.getMnemonicString(), refs);
				n++;
			}
		}
		try (PrintWriter pw = new PrintWriter(new OutputStreamWriter(
			new FileOutputStream(new File(outDir, stem + "-funcs.tsv")), "UTF-8"))) {
			pw.println("entry\tname\tbytes\tinsns");
			for (Function f : fm.getFunctions(true)) {
				if (!(f.getEntryPoint() instanceof SegmentedAddress)) {
					continue;
				}
				int k = 0;
				for (Instruction i : listing.getInstructions(f.getBody(), true)) {
					k++;
				}
				pw.printf("%s\t%s\t%d\t%d%n", rel(f.getEntryPoint()), f.getName(), f.getBody().getNumAddresses(), k);
			}
		}
		println(String.format("[insns] %d instructions written to %s", n, outDir));
	}

	private static byte[] maskOf(Instruction ins, int len) {
		byte[] m = new byte[len];
		try {
			InstructionPrototype p = ins.getPrototype();
			byte[] im = p.getInstructionMask().getBytes();
			for (int i = 0; i < len; i++) {
				m[i] = i < im.length ? im[i] : 0;
			}
		}
		catch (Exception e) {
			Arrays.fill(m, (byte) 0xFF);
		}
		return m;
	}

	private static String hex(byte[] b) {
		StringBuilder s = new StringBuilder();
		for (byte x : b) {
			s.append(String.format("%02X", x & 0xFF));
		}
		return s.toString();
	}

	// ---------------------------------------------------------------- helpers (shared shape)

	private void refuseInsideGitTree(File out) {
		for (File d = out.getAbsoluteFile(); d != null; d = d.getParentFile()) {
			if (new File(d, ".git").exists()) {
				throw new IllegalArgumentException("refusing to write an instruction table inside a git work tree: " + d);
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
