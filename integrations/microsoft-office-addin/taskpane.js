Office.onReady((info) => {
  if (info.host === Office.HostType.Word) {
    document.getElementById('runOcr').onclick = runOcr;
  }
});

async function runOcr() {
  const input = document.getElementById('imageInput');
  if (!input.files || input.files.length === 0) {
    show('Selecciona una imagen primero.');
    return;
  }

  const file = input.files[0];
  const bytes = new Uint8Array(await file.arrayBuffer());

  // TODO: replace with real locr.wasm call once ocrs is wired in locr-core.
  const text = await imageToTextPlaceholder(bytes);

  show(text);

  await Word.run(async (context) => {
    context.document.body.insertParagraph(text, Word.InsertLocation.end);
    await context.sync();
  });
}

// TODO: load locr.wasm and wire to imageToText(bytes)
async function imageToTextPlaceholder(bytes) {
  console.log('OCR placeholder received', bytes.length, 'bytes');
  return `[placeholder OCR] ${bytes.length} bytes procesados. Reemplazar con locr.wasm.`;
}

function show(text) {
  document.getElementById('output').textContent = text;
}
