export class Downloader {
  static download(name: string, content: string | Uint8Array, type: string) {
    const downloadLink = document.createElement('a');
    downloadLink.href = window.URL.createObjectURL(new Blob([content], { type }));
    downloadLink.setAttribute('download', name);

    document.body.appendChild(downloadLink);
    downloadLink.click();
    document.body.removeChild(downloadLink);
  }
}
