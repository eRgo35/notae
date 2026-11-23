# Store kit

Everything needed to redo the Google Play listing graphics, screenshots and videos for a new release, with the same demo library and data.

| Path | What |
|---|---|
| `graphics/icon-512.png` | Play app icon: `ic_launcher_foreground.png` on `#153976`, 512×512. |
| `graphics/feature-1024x500.png` | Play feature graphic. |
| `samples/*.pdf` | Demo library: 12 public-domain hymns, engraved with LilyPond. Use these exact files: `demo-data.json` refers to them by SHA-256. |
| `samples/src/*.ly` | LilyPond sources. Rebuilding gives different PDF bytes, so re-export `demo-data.json` after a rebuild. |
| `demo-data.json` | Notae **Export sets** file: favorites, the "Christmas Midnight Mass" set and its set note. |
| `screenshots/phone/` | 1080×2160 phone screenshots (Galaxy S25 FE), status bar blanked. |
| `screenshots/tablet/` | Tablet screenshots (Galaxy Tab S10 FE+, 1800×2880, light theme via `adb shell cmd uimode night no`), status bar blanked. |
| `videos/` | Short screen recordings, status bar blanked. |

## Demo setup on a device

1. Copy the samples: `adb push docs/store/samples/. "/sdcard/Documents/Notae samples/"` (only the PDFs; skip `src/`).
2. In Notae: **Settings → Library folder → Change → Documents/Notae samples → Use this folder → Allow**, then **Scan library**. Check that the count is 12; scan again if not.
3. `adb push docs/store/demo-data.json /sdcard/Download/` and **Settings → Import sets** that file.
4. Optional extras not in the export: open a few scores so **Recent** fills.

Demo content, if you rebuild it by hand:
- Favorites: Adeste fideles, Old Hundredth, Silent Night, Veni Emmanuel, Gaudete.
- Set **Christmas Midnight Mass**, 24 Dec 2026: 1. Adeste fideles, 2. Puer natus in Bethlehem, 3. Silent Night (set note "Verse 2: sopranos only, a little slower."), 4. text note "Homily", 5. Gaudete.
- Search screenshot query: `gott`.

## Capture

- Play rejects images over 2:1, so on a 19.5:9 phone run `adb shell wm size 1080x2160` first and `adb shell wm size reset` afterwards.
- Samsung One UI ignores SystemUI demo mode, so blank the status bar after capture: read its height with `adb shell dumpsys window | grep statusBars` (97 px on the S25 FE, 60 px on the Tab S10 FE+), then
  `magick raw.png -fill "<colour at x=5 just below the bar>" -draw "rectangle 0,0,<w-1>,<h-1>" out.png`
  and for videos `ffmpeg -i raw.mp4 -vf "drawbox=x=0:y=0:w=iw:h=<h>:color=0x<hex>:t=fill" -c:v libx264 -crf 20 -pix_fmt yuv420p -an out.mp4`.
- Screenshots: `adb exec-out screencap -p > shot.png`. Videos: `adb shell screenrecord --time-limit 20 /sdcard/x.mp4`, then pull and delete it.
- Afterwards, point Notae back at the real library and scan.
