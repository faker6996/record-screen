import sharp from 'sharp';
import fs from 'fs';

sharp('/tmp/icon-tools/icon.svg')
  .png()
  .toFile('/tmp/icon-tools/icon.png')
  .then(() => console.log('Successfully converted SVG to PNG.'))
  .catch(err => {
    console.error('Error converting:', err);
    process.exit(1);
  });
