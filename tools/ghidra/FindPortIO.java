// SPDX-License-Identifier: MIT
// Copyright (c) 2026 TD2192-Modern contributors
//
// Lists every IN / OUT / INS / OUTS / INT instruction in the program's code blocks with the
// port or interrupt resolved where a short backward scan can do it:
//   - port: immediate operand, or DX traced back to "mov dx,imm" / "mov dx,[mem]" through
//     inc/dec/add/sub dx,imm (reported as e.g. 3DA, or [3C43:9B90]+6 for a variable base)
//   - for INT: the AH value (from mov ah,imm / mov ax,imm / xor ah,ah)
//   - for OUT: the AL value when it was loaded with an immediate
// The scan stays inside straight-line code (it stops at labels that are jump targets).
//
// CSV columns: address, ghidra_address, segment, function, function_entry, mnemonic,
//   operands, port_or_int, ah, al
//
// Arguments: [output.csv]   default: <project parent>/exports/<exe>-portio.csv
//
// @category TD2192
// @author TD2192-Modern

import java.io.*;
import java.util.*;

import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.*;
import ghidra.program.model.lang.OperandType;
import ghidra.program.model.listing.*;
import ghidra.program.model.mem.MemoryBlock;
import ghidra.program.model.scalar.Scalar;

public class FindPortIO extends GhidraScript {

	private int imageBase;
	private static final int WINDOW = 16;

	@Override
	public void run() throws Exception {
		imageBase = findImageBase();
		File out = getScriptArgs().length > 0 ? new File(getScriptArgs()[0])
				: defaultExport(currentProgram.getName().replaceAll("\\.[^.]*$", "") + "-portio.csv");
		out.getAbsoluteFile().getParentFile().mkdirs();
		Map<String, Integer> summary = new TreeMap<>();
		int n = 0;
		try (PrintWriter pw = new PrintWriter(new OutputStreamWriter(new FileOutputStream(out), "UTF-8"))) {
			pw.println("address,ghidra_address,segment,function,function_entry,mnemonic,operands,port_or_int,ah,al");
			for (MemoryBlock b : currentProgram.getMemory().getBlocks()) {
				if (!b.isExecute() || !b.isInitialized() || !(b.getStart() instanceof SegmentedAddress)) {
					continue;
				}
				InstructionIterator it = currentProgram.getListing()
						.getInstructions(new AddressSet(b.getStart(), b.getEnd()), true);
				while (it.hasNext()) {
					Instruction ins = it.next();
					String m = ins.getMnemonicString().toUpperCase();
					String base = m.replace(".REP", "").replace(".REPE", "").replace(".REPNE", "");
					boolean io = base.equals("IN") || base.equals("OUT") || base.startsWith("INS") ||
						base.startsWith("OUTS");
					boolean intr = base.equals("INT") || base.equals("INT3") || base.equals("INTO");
					if (!io && !intr) {
						continue;
					}
					String ops = operands(ins);
					String target = "", ah = "", al = "";
					if (intr) {
						target = base.equals("INT") ? hex(scalarOperand(ins, 0)) : base;
						ah = traceAh(ins);
					}
					else {
						target = portOf(ins, base);
						if (base.equals("OUT") || base.startsWith("OUTS")) {
							al = traceAl(ins);
						}
					}
					Function f = getFunctionContaining(ins.getAddress());
					pw.println(String.join(",", rel(ins.getAddress()), ins.getAddress().toString(),
						rel(ins.getAddress()).substring(0, 4), f == null ? "" : csv(f.getName()),
						f == null ? "" : rel(f.getEntryPoint()), base, csv(ops), csv(target), ah, al));
					String key = (intr ? "INT " : base + " ") + target + (intr && !ah.isEmpty() ? " AH=" + ah : "");
					summary.merge(key, 1, Integer::sum);
					n++;
				}
			}
		}
		for (var e : summary.entrySet()) {
			println(String.format("[portio] %-28s %d", e.getKey(), e.getValue()));
		}
		println("[portio] wrote " + n + " sites to " + out);
	}

	private static String operands(Instruction ins) {
		StringBuilder sb = new StringBuilder();
		for (int i = 0; i < ins.getNumOperands(); i++) {
			if (i > 0) {
				sb.append(',');
			}
			sb.append(ins.getDefaultOperandRepresentation(i));
		}
		return sb.toString();
	}

	private static Long scalarOperand(Instruction ins, int i) {
		if (i >= ins.getNumOperands()) {
			return null;
		}
		Scalar s = ins.getScalar(i);
		return s == null ? null : s.getUnsignedValue();
	}

	private static String hex(Long v) {
		return v == null ? "?" : String.format("%X", v);
	}

	/** Port of an IN/OUT/INS/OUTS: immediate, or DX traced back. */
	private String portOf(Instruction ins, String base) {
		for (int i = 0; i < ins.getNumOperands(); i++) {
			int t = ins.getOperandType(i);
			if ((t & OperandType.SCALAR) != 0 && (t & OperandType.DYNAMIC) == 0) {
				return hex(scalarOperand(ins, i));
			}
		}
		return traceDx(ins);
	}

	private boolean isJumpTarget(Instruction ins) {
		for (var r : getReferencesTo(ins.getAddress())) {
			if (r.getReferenceType().isFlow() && !r.getReferenceType().isCall()) {
				return true;
			}
		}
		return false;
	}

	private String traceDx(Instruction from) {
		long delta = 0;
		Instruction p = from;
		for (int k = 0; k < WINDOW; k++) {
			if (isJumpTarget(p) && k > 0) {
				break;
			}
			p = p.getPrevious();
			if (p == null) {
				break;
			}
			String m = p.getMnemonicString().toUpperCase().replace(".REP", "");
			String op0 = p.getNumOperands() > 0 ? p.getDefaultOperandRepresentation(0).toUpperCase() : "";
			if (m.equals("OUT") || m.equals("IN") || m.startsWith("OUTS") || m.startsWith("INS") ||
				m.equals("PUSH") || m.equals("CMP") || m.equals("TEST")) {
				continue; // reads DX as a port or operand; does not change it
			}
			if (!op0.equals("DX") && !op0.equals("DL") && !op0.equals("DH") && !m.equals("CWD") &&
				!m.equals("MUL") && !m.equals("DIV") && !m.equals("IMUL") && !m.equals("IDIV") &&
				!m.startsWith("CALL") && !m.equals("POP")) {
				continue;
			}
			if (m.equals("POP") && !op0.equals("DX")) {
				continue;
			}
			if (op0.equals("DX")) {
				switch (m) {
					case "INC":
						delta++;
						continue;
					case "DEC":
						delta--;
						continue;
					case "ADD":
					case "SUB": {
						Long v = scalarOperand(p, 1);
						if (v == null) {
							return "DX(?)";
						}
						delta += m.equals("ADD") ? v : -v;
						continue;
					}
					case "MOV": {
						int t = p.getOperandType(1);
						if ((t & OperandType.SCALAR) != 0 && (t & OperandType.DYNAMIC) == 0) {
							return String.format("%X", (scalarOperand(p, 1) + delta) & 0xFFFF);
						}
						String src = p.getDefaultOperandRepresentation(1);
						return "DX=" + src + (delta != 0 ? String.format("%+d", delta) : "");
					}
					default:
						return "DX(" + m + ")";
				}
			}
			return "DX(" + m + " " + op0 + ")";
		}
		return "DX(?)";
	}

	private String traceAh(Instruction from) {
		Instruction p = from;
		for (int k = 0; k < WINDOW; k++) {
			if (isJumpTarget(p) && k > 0) {
				break;
			}
			p = p.getPrevious();
			if (p == null) {
				break;
			}
			String m = p.getMnemonicString().toUpperCase();
			String op0 = p.getNumOperands() > 0 ? p.getDefaultOperandRepresentation(0).toUpperCase() : "";
			if (m.startsWith("CALL") || m.equals("INT")) {
				return "";
			}
			if (op0.equals("AH")) {
				if (m.equals("MOV")) {
					Long v = immediate(p, 1);
					return v == null ? "?" : String.format("%02X", v & 0xFF);
				}
				if (m.equals("XOR") && "AH".equalsIgnoreCase(p.getDefaultOperandRepresentation(1))) {
					return "00";
				}
				return "?";
			}
			if (op0.equals("AX")) {
				if (m.equals("MOV")) {
					Long v = immediate(p, 1);
					return v == null ? "?" : String.format("%02X", (v >> 8) & 0xFF);
				}
				if (m.equals("XOR") && "AX".equalsIgnoreCase(p.getDefaultOperandRepresentation(1))) {
					return "00";
				}
				return "?";
			}
		}
		return "";
	}

	private String traceAl(Instruction from) {
		Instruction p = from;
		for (int k = 0; k < 4; k++) {
			if (isJumpTarget(p) && k > 0) {
				break;
			}
			p = p.getPrevious();
			if (p == null) {
				break;
			}
			String m = p.getMnemonicString().toUpperCase();
			String op0 = p.getNumOperands() > 0 ? p.getDefaultOperandRepresentation(0).toUpperCase() : "";
			if (op0.equals("AL") || op0.equals("AX")) {
				if (m.equals("MOV")) {
					Long v = immediate(p, 1);
					return v == null ? "" : String.format("%02X", v & 0xFF);
				}
				return "";
			}
		}
		return "";
	}

	private static Long immediate(Instruction p, int i) {
		if (i >= p.getNumOperands()) {
			return null;
		}
		int t = p.getOperandType(i);
		if ((t & OperandType.SCALAR) != 0 && (t & OperandType.DYNAMIC) == 0 &&
			(t & OperandType.ADDRESS) == 0) {
			return scalarOperand(p, i);
		}
		return null;
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
