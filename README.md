# tdu2-runtime-patch

Runtime patch layer for Test Drive Unlimited 2, focused on modding/debugging stability.

This allows for modifying runtime code or hooking functions, without the game forcibly shutting down, as well as features only possible via runtime patching.

The project does **not** patch anti-piracy or DRM systems. Intended use is preservation/modding/debugging on legitimate game installs.

## How it works

The project uses a proxy `version.dll` that forwards version API calls to Windows system providers (`kernelbase`/`kernel32`), then applies runtime patches after the game is loaded.

No files are modified on disk.

### Features

- **CameraFix**
  Patches camera bugs, applies frame-time–correct movement, and exterior + off-road camera shake/jitter/vibration.

- **DLC Car Dealer Fix**
  Allows offline purchase of DLC cars in dealerships by treating them as normal vehicles.

- **Change FOV**
  Runtime hook that allows you to change the game's Field of View by a configurable value.

- **Overlay runtime panel**
  In-game menu that allows you to configure patches and FOV on the fly.
  Press `F8` to toggle panel visibility.
  Any changes from the panel are written back to `tdu2-runtime-patch.ini`.

- **AntiTamperFix**
  Disables SecuROM anti-debug triggers and prevents forced shutdowns during runtime modification.
  This patch is required for patch stability and is always enabled.

### Compatibility

Validated on `Steam version, Update v034 DLC2 Build16 - EU` (sha1:`45bfdfe6cb600a32f9c9516bf34e62bea5af2a6`)

> [!WARNING]
> Offsets are version-specific. Using a different executable may cause crashes or undefined behavior.

## Installation

[Download the DLL here](https://github.com/hkAlice/tdu2-runtime-patch/releases), or compile it locally.

1. Copy this project's `version.dll` and `tdu2-runtime-patch.ini` next to `TestDrive2.exe` (game folder).
2. (Optional) Edit `tdu2-runtime-patch.ini` in that folder.
3. Launch the game. You may press `F8` to configure the patches.

Logs are written to `tdu2-runtime-patch.log`.

## Configuration

If the config file does not exist, a default config 
`tdu2-runtime-patch.ini` is generated on first launch.

Supported values for booleans: `1/0`, `true/false`, `yes/no`, `on/off`

`AntiTamperEnabled` is enforced to `1`. If set to `0`, it is ignored to avoid crash-prone states.

Example:

```ini
[Patch]
AntiTamperEnabled = 1
DlcCarDealerFixEnabled = 1
CameraFixEnabled = 1
CameraShakeFixEnabled = 1
StartupDelaySeconds = 5

[FOV]
Enabled = 1
Multiplier = 1.2

[Overlay]
D3D9OverlayEnabled = 0
```

> [!CAUTION]
> Disabling AntiTamperFix may cause SecuROM to trigger its security module, causing crashes or odd behaviour.

## License

This project is licensed under MIT. See [LICENSE](LICENSE).

### Community stance

- Keep it free: please do not paywall this project (or derivative builds) as a closed paid product.
- Keep attribution intact: if you redistribute binaries or modified builds, include clear credit to this project and a link to the source repository.
- Keep notices intact: MIT license and copyright notices must remain included in redistributions.

## Build

```shell
rustup target add i686-pc-windows-msvc
cargo build --release --target i686-pc-windows-msvc
```