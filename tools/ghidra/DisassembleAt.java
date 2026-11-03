// SPDX-License-Identifier: MIT
// Copyright (c) 2026 TD2192-Modern contributors
//
// Forces disassembly at given image-relative addresses (ADG 7.8) and regrows the bodies of the
// functions that contain or precede them. Use it where auto-analysis stopped early, for example
// a routine whose tail was left as undefined bytes or data (episode-deltas.md section 10: E2
// Msn_UpdateB after td2:2236:27E8). Re-run SetupBp7Program afterwards so the new code gets the
// DS value, the near-call overrides and the follow-up seeds.
//
// Arguments: at:SSSS:OOOO (repeatable) [clear:SSSS:OOOO-SSSS:OOOO] (repeatable)
//   at     disassemble (flow-following) from this address; defined data at the address is
//          cleared first. The containing or preceding function body is then regrown.
//   clear  clear defined data (not instructions) in this range before disassembling.
// key=value also works from the GUI.
//
// @category TD2192
// @author TD2192-Modern

import java.util.*;

import ghidra.app.cmd.disassemble.DisassembleCommand;
import ghidra.app.cmd.function.CreateFunctionCmd;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.*;
import ghidra.program.model.listing.*;
import ghidra.program.model.mem.MemoryBlock;

public class DisassembleAt extends GhidraScript {

	private int imageBase;

	@Override
	public void run() throws Exception {
		imageBase = findImageBase();
		List<Address> starts = new ArrayList<>();
		for (String a : getScriptArgs()) {
			if (opt(a, "at") != null) {
				starts.add(relAddr(opt(a, "at")));
			}
			else if (opt(a, "clear") != null) {
				String[] p = opt(a, "clear").split("-");
				clearData(relAddr(p[0]), relAddr(p[1]));
			}
		}
		Listing listing = currentProgram.getListing();
		for (Address s : starts) {
			Data d = listing.getDataContaining(s);
			if (d != null && d.isDefined()) {
				listing.clearCodeUnits(d.getAddress(), d.getMaxAddress(), false);
				println("[disasm] cleared data " + rel(d.getAddress()) + " " + d.getDataType().getName());
			}
			DisassembleCommand cmd = new DisassembleCommand(s, null, true);
			cmd.applyTo(currentProgram, monitor);
			AddressSetView got = cmd.getDisassembledAddressSet();
			println(String.format("[disasm] %s: %d bytes disassembled%s", rel(s), got == null ? 0 : got.getNumAddresses(),
				cmd.getStatusMsg() == null ? "" : " (" + cmd.getStatusMsg() + ")"));
			if (got != null && !got.isEmpty()) {
				Address stop = got.getMaxAddress().next();
				CodeUnit cu = stop == null ? null : listing.getCodeUnitContaining(stop);
				if (cu != null) {
					println(String.format("[disasm]   stopped before %s: %s", rel(stop), cu.getClass().getSimpleName()
						+ (cu instanceof Data dd ? " " + dd.getDataType().getName() : "")));
				}
				Iterator<Bookmark> it = currentProgram.getBookmarkManager().getBookmarksIterator("Error");
				while (it.hasNext()) {
					Bookmark bm = it.next();
					long dist = bm.getAddress().subtract(got.getMinAddress());
					if (dist >= 0 && dist <= got.getNumAddresses() + 16) {
						println("[disasm]   error bookmark " + rel(bm.getAddress()) + ": " + bm.getComment());
					}
				}
			}
			Function f = currentProgram.getFunctionManager().getFunctionContaining(s);
			if (f == null) {
				Instruction prev = listing.getInstructionBefore(s);
				f = prev == null ? null : currentProgram.getFunctionManager().getFunctionContaining(prev.getAddress());
			}
			if (f != null) {
				long before = f.getBody().getNumAddresses();
				CreateFunctionCmd.fixupFunctionBody(currentProgram, f, monitor);
				println(String.format("[disasm] %s %s body %d -> %d bytes", rel(f.getEntryPoint()), f.getName(),
					before, f.getBody().getNumAddresses()));
			}
		}
	}

	private void clearData(Address s, Address e) throws Exception {
		Listing listing = currentProgram.getListing();
		int n = 0;
		for (Data d : listing.getDefinedData(new AddressSet(s, e), true)) {
			listing.clearCodeUnits(d.getAddress(), d.getMaxAddress(), false);
			n++;
		}
		println(String.format("[disasm] cleared %d data items in %s-%s", n, rel(s), rel(e)));
	}

	private static String opt(String a, String key) {
		return a.startsWith(key + ":") || a.startsWith(key + "=") ? a.substring(key.length() + 1) : null;
	}

	private Address relAddr(String relAddr) {
		String[] p = relAddr.split(":");
		int seg = Integer.parseInt(p[0], 16) + imageBase;
		int off = Integer.parseInt(p[1], 16);
		return ((SegmentedAddressSpace) currentProgram.getAddressFactory().getDefaultAddressSpace())
				.getAddress(seg, off);
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
