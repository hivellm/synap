export interface IElectronAPI {
  getVersion: () => Promise<string>;
  getPlatform: () => Promise<string>;
}

declare global {
  interface Window {
    electron: IElectronAPI;
    ipcRenderer: {
      on: (channel: string, listener: (event: any, ...args: any[]) => void) => void;
      off: (channel: string, listener: (event: any, ...args: any[]) => void) => void;
      send: (channel: string, ...args: any[]) => void;
      invoke: (channel: string, ...args: any[]) => Promise<any>;
    };
  }
}

