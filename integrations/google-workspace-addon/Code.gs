function onOpen(e) {
  DocumentApp.getUi()
    .createAddonMenu()
    .addItem('Abrir locr', 'showSidebar')
    .addToUi();
}

function onInstall(e) {
  onOpen(e);
}

function showSidebar() {
  const html = HtmlService.createHtmlOutputFromFile('sidebar')
    .setTitle('locr OCR');
  DocumentApp.getUi().showSidebar(html);
}

function insertText(text) {
  const body = DocumentApp.getActiveDocument().getBody();
  body.appendParagraph(text);
}
