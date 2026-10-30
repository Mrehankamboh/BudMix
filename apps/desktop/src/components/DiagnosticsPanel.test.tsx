import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { api } from "../lib/api";
import { DiagnosticsPanel } from "./DiagnosticsPanel";

vi.mock("../lib/api", () => ({
  api: { diagnostics: vi.fn() },
}));

const mocked = vi.mocked(api);
const REPORT = "BudMix 0.3.2 · windows backend · Windows build 26100";

let writeText: ReturnType<typeof vi.fn>;

beforeEach(() => {
  writeText = vi.fn().mockResolvedValue(undefined);
  Object.defineProperty(navigator, "clipboard", { value: { writeText }, configurable: true });
  mocked.diagnostics.mockResolvedValue(REPORT);
});

describe("DiagnosticsPanel", () => {
  it("copies the report and shows it", async () => {
    render(<DiagnosticsPanel />);
    await userEvent.click(screen.getByRole("button", { name: "Copy diagnostics" }));
    expect(await screen.findByRole("status")).toHaveTextContent("Copied.");
    expect(writeText).toHaveBeenCalledWith(REPORT);
    expect(screen.getByRole("textbox", { name: "Diagnostics" })).toHaveValue(REPORT);
  });

  it("leaves the report to copy by hand when the clipboard is blocked", async () => {
    writeText.mockRejectedValue(new Error("denied"));
    render(<DiagnosticsPanel />);
    await userEvent.click(screen.getByRole("button", { name: "Copy diagnostics" }));
    expect(await screen.findByText(/Select the text below/)).toBeInTheDocument();
    expect(screen.getByRole("textbox", { name: "Diagnostics" })).toHaveValue(REPORT);
  });

  it("says when the report can't be read", async () => {
    mocked.diagnostics.mockRejectedValue("backend stopped");
    render(<DiagnosticsPanel />);
    await userEvent.click(screen.getByRole("button", { name: "Copy diagnostics" }));
    expect(await screen.findByRole("status")).toHaveTextContent("Couldn't read diagnostics: backend stopped");
    expect(screen.queryByRole("textbox")).not.toBeInTheDocument();
  });
});
