// SPDX-License-Identifier: MIT
// Copyright (c) 2026 TD2192-Modern contributors
//
// Applies docs/re/symbols/<exe>.csv (the source of truth for names; ADG 7.3) to the program.
//   func  : disassembles and creates the function if needed, sets the name, and writes a plate
//           comment "<comment> [<confidence>, <source_task>]" (+ "signature: ..." if given)
//   data  : sets the label and applies a data type when the signature column names one of:
//           u8 i8 u16 i16 u32 i32 ptr16 farptr real48 pstring, or T[n] / string[n]
//   label : sets the label and a pre-comment
//   struct, enum : reported and skipped (no layout language yet)
//
// Arguments: [symbols.csv]
//   default: <repo>/docs/re/symbols/<exe stem>.csv, found relative to this script
//
// @category TD2192
// @author TD2192-Modern

import java.io.*;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.*;

import ghidra.app.cmd.disassemble.DisassembleCommand;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.*;
import ghidra.program.model.data.*;
import ghidra.program.model.listing.*;
import ghidra.program.model.mem.MemoryBlock;
import ghidra.program.model.symbol.*;

public class ApplySymbols extends GhidraScript {

	private int imageBase;
	private SegmentedAddressSpace space;

	@Override
	public void run() throws Exception {
		imageBase = findImageBase();
		space = (SegmentedAddressSpace) currentProgram.getAddressFactory().getDefaultAddressSpace();
		File csvFile = getScriptArgs().length > 0 ? new File(getScriptArgs()[0]) : defaultCsv();
		if (!csvFile.exists()) {
			println("[symbols] no symbol file " + csvFile + "; nothing to apply");
			return;
		}
		List<String> lines = Files.readAllLines(csvFile.toPath(), StandardCharsets.UTF_8);
		int funcs = 0, data = 0, labels = 0, skipped = 0;
		for (int ln = 1; ln < lines.size(); ln++) {
			String line = lines.get(ln);
			if (line.isBlank()) {
				continue;
			}
			List<String> c = parseCsvLine(line);
			while (c.size() < 7) {
				c.add("");
			}
			String addr = c.get(0), kind = c.get(1), name = c.get(2), sig = c.get(3),
					comment = c.get(4), conf = c.get(5), task = c.get(6);
			Address a = toGhidra(addr);
			if (a == null) {
				printerr("[symbols] line " + (ln + 1) + ": bad address " + addr);
				skipped++;
				continue;
			}
			String note = comment + (conf.isEmpty() && task.isEmpty() ? ""
					: " [" + conf + (task.isEmpty() ? "" : ", " + task) + "]");
			switch (kind) {
				case "func": {
					Function f = getFunctionAt(a);
					if (f == null) {
						if (getInstructionAt(a) == null) {
							new DisassembleCommand(a, null, true).applyTo(currentProgram, monitor);
						}
						f = createFunction(a, name);
					}
					if (f == null) {
						printerr("[symbols] could not create function at " + addr);
						skipped++;
						continue;
					}
					f.setName(name, SourceType.USER_DEFINED);
					setPlateComment(a, note.trim() + (sig.isEmpty() ? "" : "\nsignature: " + sig));
					funcs++;
					break;
				}
				case "data": {
					setLabel(a, name);
					DataType dt = dataType(sig);
					if (dt != null) {
						try {
							clearListing(a, a.add(dt.getLength() - 1));
							createData(a, dt);
						}
						catch (Exception e) {
							printerr("[symbols] " + addr + ": could not apply " + sig + ": " + e.getMessage());
						}
					}
					if (!note.isBlank()) {
						setPreComment(a, note.trim());
					}
					data++;
					break;
				}
				case "label": {
					setLabel(a, name);
					if (!note.isBlank()) {
						setPreComment(a, note.trim());
					}
					labels++;
					break;
				}
				default:
					println("[symbols] skipping " + kind + " " + name + " at " + addr);
					skipped++;
			}
		}
		println(String.format("[symbols] applied %s: %d functions, %d data, %d labels, %d skipped",
			csvFile, funcs, data, labels, skipped));
	}

	private void setLabel(Address a, String name) throws Exception {
		Symbol s = getSymbolAt(a);
		if (s != null && s.getName().equals(name)) {
			s.setPrimary();
			return;
		}
		Symbol n = createLabel(a, name, true, SourceType.USER_DEFINED);
		n.setPrimary();
	}

	/** Our CSV type vocabulary (see symbols/README.md) mapped to Ghidra data types. */
	private DataType dataType(String sig) {
		String s = sig.trim().toLowerCase();
		if (s.isEmpty()) {
			return null;
		}
		var m = java.util.regex.Pattern.compile("^(.+)\\[(\\d+)\\]$").matcher(s);
		if (m.matches()) {
			int n = Integer.parseInt(m.group(2));
			if (m.group(1).equals("string")) { // Pascal string[n]: length byte + n chars
				return new ArrayDataType(CharDataType.dataType, n + 1, 1);
			}
			DataType el = dataType(m.group(1));
			return el == null ? null : new ArrayDataType(el, n, el.getLength());
		}
		switch (s) {
			case "u8":
				return ByteDataType.dataType;
			case "i8":
				return SignedByteDataType.dataType;
			case "u16":
				return WordDataType.dataType;
			case "i16":
				return SignedWordDataType.dataType;
			case "u32":
				return DWordDataType.dataType;
			case "i32":
				return SignedDWordDataType.dataType;
			case "ptr16":
				return new PointerDataType(DataType.DEFAULT, 2);
			case "farptr":
				return new PointerDataType(DataType.DEFAULT, 4);
			case "real48":
				return new ArrayDataType(ByteDataType.dataType, 6, 1);
			case "pstring":
				return PascalString255DataType.dataType;
			default:
				return null;
		}
	}

	private Address toGhidra(String rel) {
		String[] p = rel.trim().split(":");
		if (p.length != 2) {
			return null;
		}
		try {
			int seg = Integer.parseInt(p[0], 16), off = Integer.parseInt(p[1], 16);
			return space.getAddress((imageBase + seg) & 0xFFFF, off);
		}
		catch (NumberFormatException e) {
			return null;
		}
	}

	private File defaultCsv() {
		File scriptDir = getSourceFile().getParentFile().getFile(false);
		File repo = scriptDir.getParentFile().getParentFile();
		String stem = currentProgram.getName().replaceAll("\\.[^.]*$", "").toLowerCase();
		return new File(repo, "docs/re/symbols/" + stem + ".csv");
	}

	static List<String> parseCsvLine(String line) {
		List<String> out = new ArrayList<>();
		StringBuilder cur = new StringBuilder();
		boolean q = false;
		for (int i = 0; i < line.length(); i++) {
			char ch = line.charAt(i);
			if (q) {
				if (ch == '"' && i + 1 < line.length() && line.charAt(i + 1) == '"') {
					cur.append('"');
					i++;
				}
				else if (ch == '"') {
					q = false;
				}
				else {
					cur.append(ch);
				}
			}
			else if (ch == '"') {
				q = true;
			}
			else if (ch == ',') {
				out.add(cur.toString());
				cur.setLength(0);
			}
			else {
				cur.append(ch);
			}
		}
		out.add(cur.toString());
		return out;
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
}
