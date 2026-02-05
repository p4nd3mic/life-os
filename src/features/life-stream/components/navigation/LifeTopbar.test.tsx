// @vitest-environment jsdom
import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { LifeTopbar } from "./LifeTopbar";

describe("LifeTopbar", () => {
  it("renders the Life OS badge and actions", () => {
    render(<LifeTopbar actionsNode={<button type="button">Action</button>} />);

    expect(screen.getByText(/Life OS/i)).toBeTruthy();
    expect(screen.getByRole("button", { name: "Action" })).toBeTruthy();
  });
});
