export class Downloader {
  static download(name: string, content: Uint8Array<ArrayBuffer>, type: string) {
    const downloadLink = document.createElement('a');
    downloadLink.href = window.URL.createObjectURL(new Blob([content.buffer], { type }));
    downloadLink.setAttribute('download', name);

    document.body.appendChild(downloadLink);
    downloadLink.click();
    document.body.removeChild(downloadLink);
  }
}
