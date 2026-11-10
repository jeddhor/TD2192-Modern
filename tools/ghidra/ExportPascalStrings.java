// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (c) 2026 TD2192-Modern contributors
//
// Finds plausible Pascal ShortStrings (length byte + that many printable ASCII bytes) in the
// code segments (BP7 puts string literals next to the code that uses them) and in DGROUP
// (typed constants), and writes them with their cross-references to CSV.
//
// Cross-references: Ghidra references to the string, plus instructions with an immediate
// operand equal to the string's offset that have a PUSH CS (code-segment strings) or
// PUSH DS (DGROUP strings) within three instructions, which is how BP7 passes a string
// literal by reference.
//
// The CSV contains original game text, so it is written ONLY outside any git work tree
// (default: <project parent>/exports/<exe>-strings.csv). Never commit it.
//
// CSV columns: address, ghidra_address, where (code|dgroup), length, text, xref_count,
//   xrefs (image-relative, space separated, max 20), functions (max 5)
//
// Arguments: [output.csv] [min:<n>] (default 3) [create]
//   create: define PascalString255 data at strings in undefined code-segment bytes that
//           have at least one cross-reference (needs a writable program)
//
// @category TD2192
// @author TD2192-Modern

import java.io.*;
import java.util.*;

import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.*;
import ghidra.program.model.data.PascalString255DataType;
import ghidra.program.model.lang.OperandType;
import ghidra.program.model.listing.*;
import ghidra.program.model.mem.MemoryBlock;
import ghidra.program.model.scalar.Scalar;
import ghidra.program.model.symbol.Reference;

public class ExportPascalStrings extends GhidraScript {

	private int imageBase;
	/** key = linear segment base + offset; value = referencing instructions */
	private final Map<Long, List<Address>> immRefs = new HashMap<>();

	@Override
	public void run() throws Exception {
		imageBase = findImageBase();
		File out = null;
		int min = 3;
		boolean create = false;
		for (String a : getScriptArgs()) {
			if (opt(a, "min") != null) {
				min = Integer.parseInt(opt(a, "min"));
			}
			else if (a.equals("create")) {
				create = true;
			}
			else {
				out = new File(a);
			}
		}
		if (out == null) {
			out = defaultExport(currentProgram.getName().replaceAll("\\.[^.]*$", "") + "-strings.csv");
		}
		refuseInsideGitTree(out);
		out.getAbsoluteFile().getParentFile().mkdirs();

		indexImmediates();
		int n = 0, created = 0;
		try (PrintWriter pw = new PrintWriter(new OutputStreamWriter(new FileOutputStream(out), "UTF-8"))) {
			pw.println("address,ghidra_address,where,length,text,xref_count,xrefs,functions");
			for (MemoryBlock b : currentProgram.getMemory().getBlocks()) {
				if (!b.isInitialized() || !(b.getStart() instanceof SegmentedAddress sa)) {
					continue;
				}
				boolean code = b.isExecute();
				boolean dgroup = b.getName().equals("DGROUP");
				if (!code && !dgroup) {
					continue;
				}
				byte[] d = new byte[(int) b.getSize()];
				b.getBytes(b.getStart(), d);
				int i = 0;
				while (i < d.length) {
					int len = d[i] & 0xff;
					if (len < min || i + len >= d.length || !printable(d, i + 1, len)) {
						i++;
						continue;
					}
					Address a = b.getStart().add(i);
					if (getInstructionContaining(a) != null) {
						i++;
						continue;
					}
					String text = new String(d, i + 1, len, "ISO-8859-1");
					Set<Address> xr = xrefs(a, sa.getSegment(), b.getStart().getOffset() - sa.getSegment() * 16L + i,
						code ? "CS" : "DS");
					Set<String> fns = new LinkedHashSet<>();
					StringBuilder xs = new StringBuilder();
					int k = 0;
					for (Address x : xr) {
						if (k++ < 20) {
							xs.append(k > 1 ? " " : "").append(rel(x));
						}
						Function f = getFunctionContaining(x);
						if (f != null && fns.size() < 5) {
							fns.add(f.getName());
						}
					}
					pw.println(String.join(",", rel(a), a.toString(), code ? "code" : "dgroup",
						Integer.toString(len), csv(text), Integer.toString(xr.size()), xs.toString(),
						csv(String.join(" ", fns))));
					n++;
					if (create && code && !xr.isEmpty() && getDataContaining(a) == null) {
						try {
							createData(a, new PascalString255DataType());
							created++;
						}
						catch (Exception e) {
							// overlapping data or code: leave it
						}
					}
					i += len + 1;
				}
			}
		}
		println("[strings] wrote " + n + " strings to " + out + (create ? "; created " + created + " data items" : ""));
	}

	private static boolean printable(byte[] d, int from, int len) {
		for (int k = 0; k < len; k++) {
			int c = d[from + k] & 0xff;
			if (c < 0x20 || c >= 0x7f) {
				return false;
			}
		}
		return true;
	}

	/** Immediate operands next to PUSH CS / PUSH DS, keyed by segment base + value. */
	private void indexImmediates() {
		for (MemoryBlock b : currentProgram.getMemory().getBlocks()) {
			if (!b.isExecute() || !b.isInitialized() || !(b.getStart() instanceof SegmentedAddress sa)) {
				continue;
			}
			InstructionIterator it = currentProgram.getListing()
					.getInstructions(new AddressSet(b.getStart(), b.getEnd()), true);
			while (it.hasNext()) {
				Instruction ins = it.next();
				if (!ins.getMnemonicString().equalsIgnoreCase("PUSH") || ins.getNumOperands() != 1) {
					continue;
				}
				String reg = ins.getDefaultOperandRepresentation(0).toUpperCase();
				long base;
				if (reg.equals("CS")) {
					base = sa.getSegment() * 16L;
				}
				else if (reg.equals("DS")) {
					base = -1; // resolved per DS value below
				}
				else {
					continue;
				}
				Instruction p = ins, q = ins;
				List<Instruction> near = new ArrayList<>();
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
				for (Instruction n : near) {
					for (int i = 0; i < n.getNumOperands(); i++) {
						int t = n.getOperandType(i);
						if ((t & OperandType.SCALAR) == 0 || (t & OperandType.DYNAMIC) != 0) {
							continue;
						}
						Scalar s = n.getScalar(i);
						if (s == null) {
							continue;
						}
						long segBase = base;
						if (segBase < 0) {
							var ds = currentProgram.getProgramContext()
									.getValue(currentProgram.getProgramContext().getRegister("DS"),
										n.getAddress(), false);
							if (ds == null) {
								continue;
							}
							segBase = ds.longValue() * 16L;
						}
						immRefs.computeIfAbsent(segBase + (s.getUnsignedValue() & 0xffff),
							x -> new ArrayList<>()).add(n.getAddress());
					}
				}
			}
		}
	}

	private Set<Address> xrefs(Address a, int seg, long off, String viaReg) {
		Set<Address> out = new TreeSet<>();
		for (Reference r : getReferencesTo(a)) {
			out.add(r.getFromAddress());
		}
		List<Address> l = immRefs.get(seg * 16L + off);
		if (l != null) {
			out.addAll(l);
		}
		return out;
	}

	// ---------------------------------------------------------------- helpers (shared shape)

	/** Value of a "key:value" or "key=value" argument, else null. */
	private static String opt(String a, String key) {
		return a.startsWith(key + ":") || a.startsWith(key + "=") ? a.substring(key.length() + 1) : null;
	}

	private void refuseInsideGitTree(File out) {
		for (File d = out.getAbsoluteFile().getParentFile(); d != null; d = d.getParentFile()) {
			if (new File(d, ".git").exists()) {
				throw new IllegalArgumentException("refusing to write game text inside a git work tree: " + d);
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

	private static String csv(String s) {
		if (s.contains(",") || s.contains("\"") || s.contains("\n")) {
			return "\"" + s.replace("\"", "\"\"") + "\"";
		}
		return s;
	}
}
