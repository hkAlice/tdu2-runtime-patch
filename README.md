# tdu2-runtime-patch

Runtime patch layer for Test Drive Unlimited 2, focused on modding/debugging stability.

This allows for modifying runtime code or hooking functions, without the game forcibly shutting down. Other fixes are also included, and can be enabled/disabled as needed.

The project does **not** patch anti-piracy or DRM systems. Intended use is preservation/modding/debugging on legitimate game installs.

## How it works

The project uses a proxy `version.dll` that forwards version API calls to Windows system providers (`kernelbase`/`kernel32`), then applies runtime patches after the game is loaded.

No files are modified on disk.

### Patch groups

- **AntiTamper**
  Disables anti-debug triggers and prevents forced shutdowns during runtime modification.

- **CameraFix**
  Patches camera bugs, fixes exterior camera jitter and applies frame-time–correct movement.

- **Change FOV**
  Runtime hook that multiplies the game's Field of View by a configurable value.

### Compatibility

Validated on `Steam version, Update v034 DLC2 Build16 - EU` (sha1:`45bfdfe6cb600a32f9c9516bf34e62bea5af2a6`)

**(!)** Offsets are version-specific. Using a different executable may cause crashes or undefined behavior.

## Installation

[Download the DLL here](https://github.com/hkAlice/tdu2-runtime-patch/releases), or compile it locally.

1. Copy this project's `version.dll` and `tdu2-runtime-patch.ini` next to `TestDrive2.exe` (game folder).
2. (Optional) Edit `tdu2-runtime-patch.ini` in that folder.
3. Launch the game.

Logs are written to `tdu2-runtime-patch.log`.

## Configuration

If the config file does not exist, a default config 
`tdu2-runtime-patch.ini` is generated on first launch.

Supported values for booleans: `1/0`, `true/false`, `yes/no`, `on/off`

Example:

```ini
[Patch]
AntiTamperEnabled = 1
CameraFixEnabled = 1
StartupDelaySeconds = 5

[FOV]
Enabled = 1
Multiplier = 1.2
```

## License

This project is licensed under MIT. See [LICENSE](LICENSE).

## Build

```shell
rustup target add i686-pc-windows-msvc
cargo build --release --target i686-pc-windows-msvc
```