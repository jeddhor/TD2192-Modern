// SPDX-License-Identifier: MIT
// Copyright (c) 2026 TD2192-Modern contributors
//
// Decompiles functions to one C file each: <outdir>/<SSSS>_<OOOO>_<name>.c, with an
// image-relative address in the name and a header line.
//
// Decompiler output is derived from the original program. It is written ONLY outside any
// git work tree (default: <project parent>/exports/<exe>/). Never commit it, and never
// paste or transliterate it into the port (AGENTS.md rules 3 and 4).
//
// Arguments: [outdir] [filter:<regex>] [timeout:<seconds>] (default 60)
//   filter is searched for in "SSSS:OOOO name" (image-relative address and function name),
//   e.g. filter:3A9A: for the System unit, or filter:Random. (key=value also works when the
//   script is run from the GUI; analyzeHeadless.bat splits arguments at '=' and eats '^'.)
//
// @category TD2192
// @author TD2192-Modern

import java.io.*;
import java.util.regex.Pattern;

import ghidra.app.decompiler.*;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.*;
import ghidra.program.model.listing.Function;
import ghidra.program.model.mem.MemoryBlock;

public class ExportDecompiled extends GhidraScript {

	private int imageBase;

	@Override
	public void run() throws Exception {
		imageBase = findImageBase();
		String stem = currentProgram.getName().replaceAll("\\.[^.]*$", "");
		File outDir = null;
		Pattern filter = null;
		int timeout = 60;
		for (String a : getScriptArgs()) {
			if (opt(a, "filter") != null) {
				filter = Pattern.compile(opt(a, "filter"));
			}
			else if (opt(a, "timeout") != null) {
				timeout = Integer.parseInt(opt(a, "timeout"));
			}
			else {
				outDir = new File(a);
			}
		}
		if (outDir == null) {
			outDir = defaultExport(stem);
		}
		refuseInsideGitTree(outDir);
		outDir.mkdirs();

		DecompInterface ifc = new DecompInterface();
		DecompileOptions opts = new DecompileOptions();
		ifc.setOptions(opts);
		ifc.toggleCCode(true);
		ifc.toggleSyntaxTree(false);
		if (!ifc.openProgram(currentProgram)) {
			printerr("decompiler failed to open program: " + ifc.getLastMessage());
			return;
		}
		int ok = 0, failed = 0;
		try {
			for (Function f : currentProgram.getFunctionManager().getFunctions(true)) {
				monitor.checkCancelled();
				if (!(f.getEntryPoint() instanceof SegmentedAddress)) {
					continue;
				}
				MemoryBlock blk = currentProgram.getMemory().getBlock(f.getEntryPoint());
				if (blk == null || !blk.isExecute() || !blk.isInitialized()) {
					continue;
				}
				String r = rel(f.getEntryPoint());
				if (filter != null && !filter.matcher(r + " " + f.getName()).find()) {
					continue;
				}
				DecompileResults res = ifc.decompileFunction(f, timeout, monitor);
				String fileName = r.replace(':', '_') + "_" + f.getName().replaceAll("[^A-Za-z0-9_]", "_") + ".c";
				try (PrintWriter pw = new PrintWriter(new OutputStreamWriter(
					new FileOutputStream(new File(outDir, fileName)), "UTF-8"))) {
					pw.printf("// %s:%s %s (Ghidra %s). Decompiler output: do not commit.%n", stem, r,
						f.getName(), f.getEntryPoint());
					if (res != null && res.decompileCompleted() && res.getDecompiledFunction() != null) {
						pw.print(res.getDecompiledFunction().getC());
						ok++;
					}
					else {
						pw.println("// decompilation failed: " + (res == null ? "?" : res.getErrorMessage()));
						failed++;
					}
				}
			}
		}
		finally {
			ifc.dispose();
		}
		println(String.format("[decompile] %d functions written to %s (%d failed)", ok, outDir, failed));
	}

	// ---------------------------------------------------------------- helpers (shared shape)

	/** Value of a "key:value" or "key=value" argument, else null. */
	private static String opt(String a, String key) {
		return a.startsWith(key + ":") || a.startsWith(key + "=") ? a.substring(key.length() + 1) : null;
	}

	private void refuseInsideGitTree(File out) {
		for (File d = out.getAbsoluteFile(); d != null; d = d.getParentFile()) {
			if (new File(d, ".git").exists()) {
				throw new IllegalArgumentException("refusing to write decompiler output inside a git work tree: " + d);
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
