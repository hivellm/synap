/**
 * IPC Bridge for communication between renderer and main process
 */

export interface IpcBridge {
  // App info
  getVersion(): Promise<string>;
  getPlatform(): Promise<string>;

  // File operations
  openFileDialog(options?: {
    title?: string;
    filters?: { name: string; extensions: string[] }[];
    defaultPath?: string;
  }): Promise<string | null>;

  saveFileDialog(options?: {
    title?: string;
    filters?: { name: string; extensions: string[] }[];
    defaultPath?: string;
  }): Promise<string | null>;

  readFile(path: string): Promise<string>;
  writeFile(path: string, content: string): Promise<void>;

  // Window operations
  minimizeWindow(): void;
  maximizeWindow(): void;
  closeWindow(): void;
  toggleMaximize(): void;
  isMaximized(): Promise<boolean>;

  // System
  showNotification(title: string, body: string, options?: NotificationOptions): void;
  openExternal(url: string): Promise<void>;
}

class IpcBridgeImpl implements IpcBridge {
  private ipcRenderer = window.ipcRenderer;

  async getVersion(): Promise<string> {
    return this.ipcRenderer.invoke('app:getVersion');
  }

  async getPlatform(): Promise<string> {
    return this.ipcRenderer.invoke('app:getPlatform');
  }

  async openFileDialog(options?: {
    title?: string;
    filters?: { name: string; extensions: string[] }[];
    defaultPath?: string;
  }): Promise<string | null> {
    return this.ipcRenderer.invoke('dialog:openFile', options);
  }

  async saveFileDialog(options?: {
    title?: string;
    filters?: { name: string; extensions: string[] }[];
    defaultPath?: string;
  }): Promise<string | null> {
    return this.ipcRenderer.invoke('dialog:saveFile', options);
  }

  async readFile(path: string): Promise<string> {
    return this.ipcRenderer.invoke('fs:readFile', path);
  }

  async writeFile(path: string, content: string): Promise<void> {
    return this.ipcRenderer.invoke('fs:writeFile', path, content);
  }

  minimizeWindow(): void {
    this.ipcRenderer.send('window:minimize');
  }

  maximizeWindow(): void {
    this.ipcRenderer.send('window:maximize');
  }

  toggleMaximize(): void {
    this.ipcRenderer.send('window:maximize');
  }

  closeWindow(): void {
    this.ipcRenderer.send('window:close');
  }

  async isMaximized(): Promise<boolean> {
    return this.ipcRenderer.invoke('window:isMaximized');
  }

  showNotification(title: string, body: string, options?: NotificationOptions): void {
    this.ipcRenderer.send('notification:show', { title, body, ...options });
  }

  async openExternal(url: string): Promise<void> {
    return this.ipcRenderer.invoke('shell:openExternal', url);
  }
}

// Export singleton instance
export const ipcBridge: IpcBridge = new IpcBridgeImpl();

