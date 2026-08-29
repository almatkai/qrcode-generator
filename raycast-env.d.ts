/// <reference types="@raycast/api">

/* 🚧 🚧 🚧
 * This file is auto-generated from the extension's manifest.
 * Do not modify manually. Instead, update the `package.json` file.
 * 🚧 🚧 🚧 */

/* eslint-disable @typescript-eslint/ban-types */

type ExtensionPreferences = {
  /** Default QR Color - Default foreground color (hex) for generated QR codes */
  "defaultColor": string
}

/** Preferences accessible in all the extension's commands */
declare type Preferences = ExtensionPreferences

declare namespace Preferences {
  /** Preferences accessible in the `index` command */
  export type Index = ExtensionPreferences & {
  /** Primary Action - Set the primary action, the other will be secondary */
  "primaryAction"?: "save" | "inline" | "copy",
  /** Default Format - Set the default format for generated QR codes */
  "defaultFormat"?: "png" | "svg"
}
  /** Preferences accessible in the `clipboard` command */
  export type Clipboard = ExtensionPreferences & {}
  /** Preferences accessible in the `selection` command */
  export type Selection = ExtensionPreferences & {}
}

declare namespace Arguments {
  /** Arguments passed to the `index` command */
  export type Index = {}
  /** Arguments passed to the `clipboard` command */
  export type Clipboard = {}
  /** Arguments passed to the `selection` command */
  export type Selection = {}
}

