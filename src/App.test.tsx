import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { App } from "./App";

describe("App", () => {
  it("renders the Remember control panel title", () => {
    render(<App />);

    expect(screen.getByRole("heading", { name: "Remember" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /record/i })).toBeInTheDocument();
  });
});
