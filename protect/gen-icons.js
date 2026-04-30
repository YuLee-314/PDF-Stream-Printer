const fs = require("fs");
const zlib = require("zlib");
const path = require("path");

const dir = path.join(__dirname, "src-tauri", "icons");
fs.mkdirSync(dir, { recursive: true });

function crc32(buf) {
  let c = 0xffffffff;
  for (let i = 0; i < buf.length; i++) {
    c ^= buf[i];
    for (let j = 0; j < 8; j++) c = (c >>> 1) ^ (c & 1 ? 0xedb88320 : 0);
  }
  return (c ^ 0xffffffff) >>> 0;
}

function chunk(type, data) {
  const l = Buffer.alloc(4);
  l.writeUInt32BE(data.length);
  const td = Buffer.concat([Buffer.from(type), data]);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(td));
  return Buffer.concat([l, td, crc]);
}

function makePNG(w, h, r, g, b, a) {
  const sig = Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]);

  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(w, 0);
  ihdr.writeUInt32BE(h, 4);
  ihdr[8] = 8; // bit depth
  ihdr[9] = 6; // RGBA
  ihdr[10] = 0;
  ihdr[11] = 0;
  ihdr[12] = 0;

  const raw = Buffer.alloc(h * (1 + w * 4));
  for (let y = 0; y < h; y++) {
    const row = y * (1 + w * 4);
    raw[row] = 0; // filter none
    for (let x = 0; x < w; x++) {
      const px = row + 1 + x * 4;
      raw[px] = r;
      raw[px + 1] = g;
      raw[px + 2] = b;
      raw[px + 3] = a;
    }
  }

  return Buffer.concat([
    sig,
    chunk("IHDR", ihdr),
    chunk("IDAT", zlib.deflateSync(raw)),
    chunk("IEND", Buffer.alloc(0)),
  ]);
}

function makeICO(png, size) {
  const hdr = Buffer.alloc(6);
  hdr.writeUInt16LE(0, 0);
  hdr.writeUInt16LE(1, 2);
  hdr.writeUInt16LE(1, 4);

  const s = size >= 256 ? 0 : size;
  const e = Buffer.alloc(16);
  e.writeUInt8(s, 0);
  e.writeUInt8(s, 1);
  e.writeUInt8(0, 2);
  e.writeUInt8(0, 3);
  e.writeUInt16LE(1, 4);
  e.writeUInt16LE(32, 6);
  e.writeUInt32LE(png.length, 8);
  e.writeUInt32LE(22, 12);

  return Buffer.concat([hdr, e, png]);
}

const png32 = makePNG(32, 32, 233, 69, 96, 255);
fs.writeFileSync(path.join(dir, "32x32.png"), png32);
fs.writeFileSync(path.join(dir, "icon.ico"), makeICO(png32, 32));

const png128 = makePNG(128, 128, 233, 69, 96, 255);
fs.writeFileSync(path.join(dir, "128x128.png"), png128);

const png256 = makePNG(256, 256, 233, 69, 96, 255);
fs.writeFileSync(path.join(dir, "128x128@2x.png"), png256);

console.log("OK: icon.ico + 3 PNGs");
