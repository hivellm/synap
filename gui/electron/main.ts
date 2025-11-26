import { app, BrowserWindow, ipcMain, dialog, shell, Notification } from 'electron';
import path from 'node:path';
import fs from 'node:fs/promises';

// The built directory structure
//
// ├─┬─┬ dist
// │ │ └── index.html
// │ │
// │ ├─┬ dist-electron
// │ │ ├── main.js
// │ │ └── preload.js
// │

process.env.DIST = path.join(__dirname, '../dist');
process.env.VITE_PUBLIC = app.isPackaged
  ? process.env.DIST
  : path.join(process.env.DIST, '../public');

let win: BrowserWindow | null = null;
// Here, you can also use other preload
const preload = path.join(__dirname, 'preload.js');
// 🚧 Use ['ENV_NAME'] avoid vite:define plugin - Vite@2.x
const VITE_DEV_SERVER_URL = process.env['VITE_DEV_SERVER_URL'];

function createWindow() {
  win = new BrowserWindow({
    width: 1200,
    height: 800,
    minWidth: 800,
    minHeight: 600,
    icon: path.join(process.env.VITE_PUBLIC || '', 'icon.png'),
    frame: false, // Custom titlebar - remove default OS frame
    transparent: false,
    backgroundColor: '#1a1a1a',
    webPreferences: {
      preload,
      nodeIntegration: false,
      contextIsolation: true,
      sandbox: false,
    },
    ...(process.platform === 'darwin' && {
      titleBarStyle: 'hiddenInset',
      trafficLightPosition: { x: 10, y: 10 },
    }),
  });

  // Test active push message to Renderer-process.
  win.webContents.on('did-finish-load', () => {
    win?.webContents.send('main-process-message', new Date().toLocaleString());
  });

  if (VITE_DEV_SERVER_URL) {
    // electron-vite-vue#298
    win.loadURL(VITE_DEV_SERVER_URL);
    // Open devTool if the app is not packaged
    win.webContents.openDevTools();
  } else {
    // win.loadFile('dist/index.html')
    win.loadFile(path.join(process.env.DIST!, 'index.html'));
  }
}

// Quit when all windows are closed, except on macOS. There, it's common
// for applications and their menu bar to stay active until the user quits
// explicitly with Cmd + Q.
app.on('window-all-closed', () => {
  if (process.platform !== 'darwin') {
    app.quit();
    win = null;
  }
});

app.on('activate', () => {
  // On OS X it's common to re-create a window in the app when the
  // dock icon is clicked and there are no other windows open.
  if (BrowserWindow.getAllWindows().length === 0) {
    createWindow();
  }
});

app.whenReady().then(createWindow);

// IPC handlers
ipcMain.handle('app:getVersion', () => {
  return app.getVersion();
});

ipcMain.handle('app:getPlatform', () => {
  return process.platform;
});

// File dialog handlers
ipcMain.handle('dialog:openFile', async (_, options?: {
  title?: string;
  filters?: { name: string; extensions: string[] }[];
  defaultPath?: string;
}) => {
  if (!win) return null;

  const result = await dialog.showOpenDialog(win, {
    title: options?.title || 'Open File',
    filters: options?.filters || [
      { name: 'All Files', extensions: ['*'] },
    ],
    defaultPath: options?.defaultPath,
    properties: ['openFile'],
  });

  if (result.canceled || result.filePaths.length === 0) {
    return null;
  }

  return result.filePaths[0];
});

ipcMain.handle('dialog:saveFile', async (_, options?: {
  title?: string;
  filters?: { name: string; extensions: string[] }[];
  defaultPath?: string;
}) => {
  if (!win) return null;

  const result = await dialog.showSaveDialog(win, {
    title: options?.title || 'Save File',
    filters: options?.filters || [
      { name: 'All Files', extensions: ['*'] },
    ],
    defaultPath: options?.defaultPath,
  });

  if (result.canceled || !result.filePath) {
    return null;
  }

  return result.filePath;
});

// File system handlers
ipcMain.handle('fs:readFile', async (_, filePath: string) => {
  try {
    const content = await fs.readFile(filePath, 'utf-8');
    return content;
  } catch (error: any) {
    throw new Error(`Failed to read file: ${error.message}`);
  }
});

ipcMain.handle('fs:writeFile', async (_, filePath: string, content: string) => {
  try {
    await fs.writeFile(filePath, content, 'utf-8');
  } catch (error: any) {
    throw new Error(`Failed to write file: ${error.message}`);
  }
});

// Window handlers
ipcMain.on('window:minimize', () => {
  if (win) {
    win.minimize();
  }
});

ipcMain.on('window:maximize', () => {
  if (win) {
    if (win.isMaximized()) {
      win.unmaximize();
    } else {
      win.maximize();
    }
  }
});

ipcMain.on('window:close', () => {
  if (win) {
    win.close();
  }
});

ipcMain.handle('window:isMaximized', () => {
  return win?.isMaximized() || false;
});

// Notification handler
ipcMain.on('notification:show', (_, options: { title: string; body: string }) => {
  if (Notification.isSupported()) {
    new Notification({
      title: options.title,
      body: options.body,
    }).show();
  }
});

// Shell handlers
ipcMain.handle('shell:openExternal', async (_, url: string) => {
  await shell.openExternal(url);
});

