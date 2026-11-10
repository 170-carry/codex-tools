export const trayPreviews = [
  ["gradientNumberPlate", 18, 18], ["gradientNumberCard", 32, 18], ["gradientNumber", 24, 18],
  ["numberProgressBar", 32, 18], ["logoProgressRing", 18, 18],
].map(([style, width, height]) => ({
  style, pixelWidth: width, pixelHeight: height,
  dataUrl: `data:image/svg+xml,${encodeURIComponent(`<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}" viewBox="0 0 ${width} ${height}"><rect width="100%" height="100%" rx="6" fill="#7775ef"/><text x="50%" y="13" text-anchor="middle" fill="white" font-family="Arial" font-size="12">97</text></svg>`)}`,
}));
