/**
 * Keep the remote session alive briefly after lock / minimize.
 * Uses HarmonyOS transient task (requestSuspendDelay); we disconnect after ~60s.
 */
import { backgroundTaskManager } from '@kit.BackgroundTasksKit';

const KEEP_MS = 60 * 1000;

export class SessionKeepAlive {
  private static active: boolean = false;
  private static requestId: number = -1;
  private static timer: number = -1;
  private static disconnectHandler: (() => void) | null = null;

  static setActive(active: boolean, onDisconnect?: () => void): void {
    SessionKeepAlive.active = active;
    if (active && onDisconnect) {
      SessionKeepAlive.disconnectHandler = onDisconnect;
    }
    if (!active) {
      SessionKeepAlive.clearBackgroundHold();
      SessionKeepAlive.disconnectHandler = null;
    }
  }

  static isActive(): boolean {
    return SessionKeepAlive.active;
  }

  /** Ability entered background (lock screen / home / switch app). */
  static onAppBackground(): void {
    if (!SessionKeepAlive.active) {
      return;
    }
    SessionKeepAlive.clearBackgroundHold();
    let waitMs = KEEP_MS;
    try {
      const delayInfo = backgroundTaskManager.requestSuspendDelay('保留远程桌面连接', () => {
        console.info('[SessionKeepAlive] suspend delay about to timeout');
        SessionKeepAlive.forceDisconnect();
      });
      SessionKeepAlive.requestId = delayInfo.requestId;
      const sysMs = delayInfo.actualDelayTime;
      if (sysMs > 0) {
        waitMs = Math.min(KEEP_MS, Math.max(3000, sysMs - 1500));
      }
      console.info(`[SessionKeepAlive] background hold id=${SessionKeepAlive.requestId} wait=${waitMs} sys=${sysMs}`);
    } catch (error) {
      console.warn('[SessionKeepAlive] requestSuspendDelay failed: ' + JSON.stringify(error));
    }
    SessionKeepAlive.timer = setTimeout(() => {
      SessionKeepAlive.forceDisconnect();
    }, waitMs);
  }

  /** Ability returned to foreground — keep connection, drop the hold timer. */
  static onAppForeground(): void {
    if (SessionKeepAlive.requestId >= 0 || SessionKeepAlive.timer !== -1) {
      console.info('[SessionKeepAlive] foreground, cancel background disconnect');
    }
    SessionKeepAlive.clearBackgroundHold();
  }

  private static forceDisconnect(): void {
    SessionKeepAlive.clearBackgroundHold();
    if (!SessionKeepAlive.active) {
      return;
    }
    console.info('[SessionKeepAlive] disconnect after background hold');
    SessionKeepAlive.active = false;
    const handler = SessionKeepAlive.disconnectHandler;
    SessionKeepAlive.disconnectHandler = null;
    if (handler) {
      handler();
    }
  }

  private static clearBackgroundHold(): void {
    if (SessionKeepAlive.timer !== -1) {
      clearTimeout(SessionKeepAlive.timer);
      SessionKeepAlive.timer = -1;
    }
    if (SessionKeepAlive.requestId >= 0) {
      try {
        backgroundTaskManager.cancelSuspendDelay(SessionKeepAlive.requestId);
      } catch (_e) {
      }
      SessionKeepAlive.requestId = -1;
    }
  }
}
