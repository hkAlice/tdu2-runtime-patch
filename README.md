# tdu2_camera_fix

Reduces exterior camera jitter in Test Drive Unlimited 2 at high resolutions and framerates.

## The problem
TDU2's exterior cameras (far, close, front) have a speed-dependent shake effect driven by a sine wave phase accumulator. At higher resolutions and fps, the effect becomes a high-frequency strobe/jitter rather than the intended effect. The cockpit and hood cameras are unaffected. The issue is present in the console versions too, just more visible at higher res and FPS.

The solution, otherwise, is to use cockpit/hood camera.

## What this does

Patches the game's camera shake system in memory at runtime:

Kills the speed input to the sine phase accumulator
Prevents the phase from being written back
Skips the shake LUT lookup for exterior cameras

Jitter is reduced but some residual motion may remain (I suspect smoothing/camera flags).
Tested on `Steam version, EU, Build 16 DLC2` - SHA1: `45bfdfe6cb600a32f9c9516bf34e62bea5af2a6`

Other versions may have different EXE offsets and are untested.

## Installation

1. Copy C:\Windows\SysWOW64\version.dll to your TDU2 game folder and rename it to version_orig.dll
2. Copy version.dll to your TDU2 game folder
3. Launch the game normally

## Uninstall

Delete both DLL files from the game folder.

## Building

```shell
rustup target add i686-pc-windows-msvc
cargo build --release --target i686-pc-windows-msvc
```

Copy `version_orig.dll` from `C:\Windows\SysWOW64\version.dll` to the game folder alongside the compiled DLL.

## Technical notes

The shake is generated in `FUN_00BBC650` via a sine phase accumulator at `EBX+0x238`, fed by a speed-scaled increment. Patched offsets:

```
Phase accumulator:
  +7BCFBE  FLD [EBX+0x174]  ->  FLDZ        (kill speed input)
  +7BD001  FST [EBX+0x238]  ->  FSTP ST0    (discard phase write)
  +7BD015  FSTP [EBX+0x238] ->  FSTP ST0    (discard phase wrap write)

Shake LUT skip:
  +851244  JZ  ->  JMP    (FUN_00C51230, always skip shake fetch)
  +851274  JZ  ->  JMP    (FUN_00C51260, always skip shake fetch)
```

The values `0x53`, `0x37` and `0x36` in particular are interesting. `0x53` has special code that avoids copying from a LUT.

`FUN_00C519A0` is quite interesting but hard to track down without a debugger attached to the game (indirect virtual dispatching). Might be worth looking into the camera flags returned here.

`FUN_00bbc650` is a *huge* function that deals with a lot of the game's driving. Camera, car physics all seem to fall into this function in some manner or another.

`FUN_00CA2130` is worth a look as hell. This is where it reads the camera mask, and has written `2.5f` to some kind of camera amplitude.
`0xD0000` scalar search could be worth a follow too since that's tied to the camera mask.

Camera mode is at `(DAT_0119674C, RVA TestDrive2.exe+0xD9674C)`
0 - Exterior Far
1 - Exterior Close
2 - Cockpit (no jitter)
3 - Hood (no jitter)
4 - Exterior front camera

0, 1, 4 all have the jitter issues and have special code handling them.

`TestDrive2.exe+D95B58` to `TestDrive2.exe+D9674C` -> browsing the memory region, this seems to be the actual camera stuff. Moving the camera around changes almost everything around this memory area.
                                                                                                                      
`TestDrive2.exe+BE6CD0` and `TestDrive2.exe+BF0630`: Actual relative camera offsets? Doesn't seem to change with rotation of the camera or position of car.