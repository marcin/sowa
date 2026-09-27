// Odtwarza wyjście programu w tempie 60 klatek/s: dzieli strumień na klatki po ESC 8
// (powrót kursora na początek płótna). Sowa nie ma jeszcze pauzy, więc tempo daje ten skrypt.
const fps = Number(process.argv[2] ?? 60);
const data = await Bun.stdin.text();
const frames = data.split("\x1b8");
for (let i = 0; i < frames.length; i++) {
  process.stdout.write((i > 0 ? "\x1b8" : "") + frames[i]);
  await Bun.sleep(1000 / fps);
}
