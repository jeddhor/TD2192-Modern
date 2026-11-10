// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (c) 2026 TD2192-Modern contributors
//
// Writes user-defined Ghidra names back to docs/re/symbols/<exe>.csv (ADG 7.3), for agents
// who named things in the GUI. Existing rows keep their kind, signature, comment, confidence
// and source_task; only the name is updated. New names become rows with an empty comment
// (comments are never exported from Ghidra, so no decompiler text can leak into the CSV).
// Rows for addresses Ghidra does not name are kept. Output is sorted by address.
//
// Arguments: [symbols.csv] [task:<task id>] [confidence:<CONFIRMED|LIKELY|HYPOTHESIS>]
//   default csv: <repo>/docs/re/symbols/<exe stem>.csv, found relative to this script
//   default confidence for new rows: HYPOTHESIS
//
// @category TD2192
// @author TD2192-Modern

import java.io.*;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.*;

import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.*;
import ghidra.program.model.listing.*;
import ghidra.program.model.mem.MemoryBlock;
import ghidra.program.model.symbol.*;

public class ExportSymbols extends GhidraScript {

	private static final String HEADER = "address,kind,name,signature,comment,confidence,source_task";
	private int imageBase;

	@Override
	public void run() throws Exception {
		imageBase = findImageBase();
		File csvFile = null;
		String task = "", conf = "HYPOTHESIS";
		for (String a : getScriptArgs()) {
			if (a.startsWith("task:") || a.startsWith("task=")) {
				task = a.substring(5);
			}
			else if (a.startsWith("confidence:") || a.startsWith("confidence=")) {
				conf = a.substring(11);
			}
			else {
				csvFile = new File(a);
			}
		}
		if (csvFile == null) {
			csvFile = defaultCsv();
		}
		TreeMap<String, List<String>> rows = new TreeMap<>();
		if (csvFile.exists()) {
			List<String> lines = Files.readAllLines(csvFile.toPath(), StandardCharsets.UTF_8);
			for (int i = 1; i < lines.size(); i++) {
				if (!lines.get(i).isBlank()) {
					List<String> c = ApplySymbolsCsv.parse(lines.get(i));
					while (c.size() < 7) {
						c.add("");
					}
					rows.put(c.get(0).toUpperCase(), c);
				}
			}
		}
		int added = 0, renamed = 0;
		SymbolIterator it = currentProgram.getSymbolTable().getAllSymbols(true);
		while (it.hasNext()) {
			Symbol s = it.next();
			if (s.getSource() != SourceType.USER_DEFINED || !s.isPrimary() ||
				!(s.getAddress() instanceof SegmentedAddress)) {
				continue;
			}
			if (s.getSymbolType() != SymbolType.FUNCTION && s.getSymbolType() != SymbolType.LABEL) {
				continue;
			}
			String addr = rel(s.getAddress());
			List<String> row = rows.get(addr);
			if (row != null) {
				if (!row.get(2).equals(s.getName())) {
					row.set(2, s.getName());
					renamed++;
				}
				continue;
			}
			String kind = s.getSymbolType() == SymbolType.FUNCTION ? "func"
					: (getDataAt(s.getAddress()) != null && getDataAt(s.getAddress()).isDefined()
							? "data" : "label");
			rows.put(addr, new ArrayList<>(List.of(addr, kind, s.getName(), "", "", conf, task)));
			added++;
		}
		try (PrintWriter pw = new PrintWriter(new OutputStreamWriter(new FileOutputStream(csvFile), "UTF-8"))) {
			pw.print(HEADER + "\n");
			for (List<String> r : rows.values()) {
				StringBuilder sb = new StringBuilder();
				for (int i = 0; i < 7; i++) {
					if (i > 0) {
						sb.append(',');
					}
					sb.append(csv(r.get(i)));
				}
				pw.print(sb + "\n");
			}
		}
		println(String.format("[symbols] %s: %d rows (%d new, %d renamed)", csvFile, rows.size(), added, renamed));
	}

	/** Minimal RFC 4180 line parser (same rules as ApplySymbols). */
	static final class ApplySymbolsCsv {
		static List<String> parse(String line) {
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
	}

	private static String csv(String s) {
		if (s.contains(",") || s.contains("\"") || s.contains("\n")) {
			return "\"" + s.replace("\"", "\"\"") + "\"";
		}
		return s;
	}

	private File defaultCsv() {
		File scriptDir = getSourceFile().getParentFile().getFile(false);
		File repo = scriptDir.getParentFile().getParentFile();
		String stem = currentProgram.getName().replaceAll("\\.[^.]*$", "").toLowerCase();
		return new File(repo, "docs/re/symbols/" + stem + ".csv");
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
}
