VICTORY DLSS FRAME GENERATION SELECTOR - 3x / 4x / 5x / 6x

WHAT THIS PACK DOES
- Opens a terminal menu before starting Victory.
- Lets you choose 3x, 4x, 5x, or 6x DLSS Frame Generation.
- Writes its settings and logs beside the launcher, not into the game dump.
- Removes the saved FPS cap for the test and blocks duplicate Victory launches.

REQUIRED
- Your own working Victory setup with the NVIDIA fork.
- Your own legally obtained game dump containing game_scarlett_master.exe.
- Your own NVIDIA Streamline SDK installation.
- A compatible NVIDIA GPU. This was tested on an RTX 4050 Laptop GPU.
- Internet access on first run if MFG Unlocker is missing. The launcher uses Victory's built-in get-mfg command to download the official v1.7.2 release and verify the archive and DLL against the author's published SHA-256 hashes.

The usual Victory locations are:
  %LOCALAPPDATA%\victory\mfg-unlocker\MFGUnlocker.dll
  %LOCALAPPDATA%\victory\nvidia-sdk\Streamline\bin\x64\sl.interposer.dll

HOW TO USE
1. Extract this zip into a folder.
2. Double-click START-HERE.cmd.
3. If victory.exe is not beside the launcher, enter the full path to your existing victory.exe. The launcher copies it into its own folder.
4. If MFG Unlocker is missing, the launcher downloads and verifies it automatically.
5. Enter the full path to your game_scarlett_master.exe. The path is saved in game-path.txt for later runs.
6. Choose 3, 4, 5, or 6.
7. Enter gameplay; DLSS Frame Generation activates when the game creates its frame-generation feature.

To try another multiplier, close Victory and run START-HERE.cmd again.

NOT INCLUDED
- Game files or the game executable
- victory.exe
- MFGUnlocker.dll
- NVIDIA SDK files

Those binaries are not redistributed in this pack. Supply your own copies and follow the licenses that apply to them.
