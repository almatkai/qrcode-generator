import assert from "node:assert/strict";
import test from "node:test";
import {
  isValidHexColor,
  normalizeHexColor,
  isLowContrast,
  buildQrOptions,
  buildSvgOptions,
  resolveColorPreference,
  DEFAULT_COLOR,
} from "../src/config";
import { isHttpUrl, appendUtmParams } from "../src/url";

test("hex color validation and normalization", () => {
  assert.equal(isValidHexColor("#abc"), true);
  assert.equal(isValidHexColor("abc"), true);
  assert.equal(isValidHexColor("#aabbcc"), true);
  assert.equal(isValidHexColor("aabbcc"), true);
  assert.equal(isValidHexColor("zzz"), false);
  assert.equal(isValidHexColor(""), false);
  assert.equal(normalizeHexColor("abc"), "#aabbcc");
  assert.equal(normalizeHexColor("#abc"), "#aabbcc");
  assert.equal(normalizeHexColor("aabbcc"), "#aabbcc");
  assert.equal(resolveColorPreference("#fff"), "#ffffff");
  assert.equal(resolveColorPreference("not-a-color"), DEFAULT_COLOR);
});

test("low contrast detection for QR readability", () => {
  assert.equal(isLowContrast("#000000"), false);
  assert.equal(isLowContrast("#ffffff"), true);
  assert.equal(isLowContrast("#ffff00"), true);
  assert.equal(isLowContrast("invalid"), false);
});

test("QR options build with correct colors and backgrounds", () => {
  const png = buildQrOptions({ color: "#ff0000", preview: false });
  assert.equal(png.color.dark, "#ff0000");
  assert.equal(png.color.light, "#00000000");
  assert.equal(png.width, 512);
  const preview = buildQrOptions({ color: "#00ff00", preview: true });
  assert.equal(preview.color.light, "#FFFFFF");
  const svg = buildSvgOptions({ color: "#0000ff" });
  assert.equal(svg.color.dark, "#0000ff");
  assert.equal(svg.color.light, "none");
  assert.equal(svg.width, 1536);
});

test("URL helpers detect http and append UTM params", () => {
  assert.equal(isHttpUrl("https://example.com"), true);
  assert.equal(isHttpUrl("http://example.com/path?q=1"), true);
  assert.equal(isHttpUrl("not a url"), false);
  assert.equal(isHttpUrl("ftp://example.com"), false);
  const withUtm = appendUtmParams("https://example.com", {
    source: "newsletter",
    medium: "email",
    campaign: "spring",
  });
  const url = new URL(withUtm);
  assert.equal(url.searchParams.get("utm_source"), "newsletter");
  assert.equal(url.searchParams.get("utm_medium"), "email");
  assert.equal(url.searchParams.get("utm_campaign"), "spring");
  // non-http content returned unchanged
  assert.equal(appendUtmParams("plain text", { source: "x" }), "plain text");
});

test("package.json declares Windows+macOS and Tezbar metadata", async () => {
  const pkg = await import("../package.json", { with: { type: "json" } });
  const data = (pkg as unknown as { default: Record<string, unknown> }).default ?? (pkg as unknown as Record<string, unknown>);
  assert.deepEqual((data as { platforms: string[] }).platforms, ["macOS", "Windows"]);
  const tezbar = (data as { tezbar: { platforms: string[]; repository: string } }).tezbar;
  assert.deepEqual(tezbar.platforms, ["macos", "windows"]);
  assert.equal(tezbar.repository, "https://github.com/almatkai/qrcode-generator");
});
