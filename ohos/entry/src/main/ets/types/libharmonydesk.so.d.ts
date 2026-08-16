/**
 * Type declarations for libharmonydesk.so native module
 * This file provides TypeScript definitions for the NAPI bindings
 */

export interface NativeVideoFrame {
  width: number;
  height: number;
  data: ArrayBuffer;
  timestamp: number;
}

export interface NativeVideoStats {
  status: number;
  packets: number;
  frames: number;
  width: number;
  height: number;
  decoded: number;
  pushed?: number;
  surface?: number;
  convertMs?: number;
  queue?: number;
}

export interface HarmonyDeskNativeModule {
  /**
   * Initialize the native module
   * @returns 0 on success, error code otherwise
   */
  init(): number;
  initDebug(): number;

  /**
   * Configure server settings
   * @param idServer - ID server address
   * @param relayServer - Relay server address
   * @param forceRelay - Whether to force relay connection
   * @param key - Encryption key
   * @returns 0 on success, error code otherwise
   */
  setServerConfig(idServer: string, relayServer: string, forceRelay: boolean, key: string): number;

  /**
   * Connect to a remote desktop
   * @param deskId - Remote desktop ID
   * @param password - Connection password
   * @returns 0 on success, error code otherwise
   */
  connect(deskId: string, password: string): number;

  /**
   * Disconnect all connections
   */
  disconnect(): void;

  /**
   * Clean up resources
   */
  cleanup(): void;

  /**
   * Get current connection status
   * @returns Connection status code (0 = disconnected, 1 = connecting, 2 = connected)
   */
  getConnectionStatus(): number;

  /**
   * Send keyboard event
   * @param keyCode - Key code
   * @param pressed - Whether key is pressed
   */
  sendKeyEvent(keyCode: number, pressed: boolean): void;

  /**
   * Send mouse move event
   * @param x - X coordinate
   * @param y - Y coordinate
   */
  sendMouseMove(x: number, y: number): void;

  /**
   * Send mouse click event
   * @param button - Button number (0 = left, 1 = middle, 2 = right)
   * @param pressed - Whether button is pressed
   */
  sendMouseClick(button: number, pressed: boolean): void;
  sendText(text: string): void;
  sendControlKey(code: number, pressed: boolean): void;
  /** modifier+key: chr ASCII or 1000+ControlKey; modifier ControlKey (Alt=1,Ctrl=4,Meta=23) */
  sendChord(chr: number, modifier: number): void;
  setImageQuality(quality: number): void;

  /**
   * Get latest video frame
   * @returns Video frame or null if no frame available
   */
  getVideoFrame(): NativeVideoFrame | null;
  getVideoStats(): NativeVideoStats;
  bindVideoSurface(surfaceId: string): number;
  unbindVideoSurface(): void;
  getLogs(): string;
  getLastError(): string | null;
  clearLogs(): void;
}

export default {} as HarmonyDeskNativeModule;
