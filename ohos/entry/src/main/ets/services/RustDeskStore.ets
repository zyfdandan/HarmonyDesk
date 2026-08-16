import preferences from '@ohos.data.preferences';
import type common from '@ohos.app.ability.common';

export interface DeviceItem {
  id: string;
  name: string;
  status: string;
  lastSeen: string;
  tags: string[];
  /** Saved remote password; used for one-tap connect. */
  password: string;
}

export interface PolicyConfig {
  autoAccept: boolean;
  clipboardSync: boolean;
  fileTransfer: boolean;
  viewOnlyDefault: boolean;
}

export interface RelayConfig {
  relayEnabled: boolean;
  directEnabled: boolean;
  selfHostEnabled: boolean;
  relayHost: string;
  apiHost: string;
  key: string;
}

export interface DeviceGroup {
  id: string;
  name: string;
  description: string;
  deviceCount: number;
}

export interface RecentSession {
  id: string;
  name: string;
  endedAt: string;
  quality: string;
  mode: string;
}

const STORE_NAME = 'rustdesk_store';
const KEY_DEVICES = 'devices';
const KEY_POLICY = 'policy';
const KEY_RELAY = 'relay';
const KEY_GROUPS = 'groups';
const KEY_SESSIONS = 'sessions';

const DEFAULT_DEVICES: DeviceItem[] = [
  { id: 'DEMO-001', name: '我的电脑', status: 'Online', lastSeen: '当前在线', tags: [], password: '' },
  { id: 'DEMO-002', name: '办公室电脑', status: 'Offline', lastSeen: '2小时前', tags: [], password: '' }
];

const DEFAULT_POLICY: PolicyConfig = {
  autoAccept: false,
  clipboardSync: true,
  fileTransfer: true,
  viewOnlyDefault: false
};

const DEFAULT_RELAY: RelayConfig = {
  relayEnabled: true,
  directEnabled: true,
  selfHostEnabled: true,
  relayHost: '',
  apiHost: '',
  key: ''
};

export function isOfficialPlaceholderHost(host: string): boolean {
  const value = host.trim().toLowerCase();
  return value.length === 0
    || value.includes('rustdesk.com/api')
    || value === 'router.rustdesk.com'
    || value === 'router.rustdesk.com:21116'
    || value === 'relay.rustdesk.org'
    || value === 'relay.rustdesk.com';
}

export function isPlaceholderKey(key: string): boolean {
  return key.trim() === '123456';
}

/** RustDesk direct IP access: IPv4, IPv4:port, or host:port (default 21118). */
export function isDirectIpId(id: string): boolean {
  const value = id.trim();
  if (value.length === 0) {
    return false;
  }
  if (/^\d{1,3}(\.\d{1,3}){3}$/.test(value)) {
    return true;
  }
  if (/^\d{1,3}(\.\d{1,3}){3}:\d{1,5}$/.test(value)) {
    return true;
  }
  // host:port where host is not all digits (not a RustDesk numeric id with colon)
  const colon = value.lastIndexOf(':');
  if (colon > 0) {
    const host = value.substring(0, colon);
    const port = value.substring(colon + 1);
    if (host.length > 0 && /^\d{1,5}$/.test(port) && !/^\d+$/.test(host)) {
      return true;
    }
  }
  return false;
}

const DEFAULT_GROUPS: DeviceGroup[] = [
  { id: 'group-onsite', name: 'On-site', description: 'Direct LAN access', deviceCount: 12 },
  { id: 'group-remote', name: 'Remote Teams', description: 'Relay only', deviceCount: 8 }
];

const DEFAULT_SESSIONS: RecentSession[] = [
  { id: 'sess-1', name: 'Design Studio', endedAt: '5 min ago', quality: '1080p', mode: 'Full Control' },
  { id: 'sess-2', name: 'Warehouse Terminal', endedAt: '2 hours ago', quality: '720p', mode: 'View Only' },
  { id: 'sess-3', name: 'Reception Tablet', endedAt: 'Yesterday', quality: '1080p', mode: 'Full Control' }
];

export class RustDeskStore {
  private prefs: preferences.Preferences | null = null;

  async init(context: common.UIAbilityContext): Promise<void> {
    if (this.prefs) {
      return;
    }
    this.prefs = await preferences.getPreferences(context, STORE_NAME);
  }

  async loadDevices(): Promise<DeviceItem[]> {
    const raw = await this.getStore().get(KEY_DEVICES, JSON.stringify(DEFAULT_DEVICES)) as string;
    const list = this.safeParse<DeviceItem[]>(raw, DEFAULT_DEVICES);
    return list.map((item: DeviceItem): DeviceItem => {
      const rawStatus = item.status ?? '';
      const status = (rawStatus.length === 0) ? 'Unknown' : rawStatus;
      return {
        id: item.id ?? '',
        name: item.name ?? item.id ?? '',
        status: status,
        lastSeen: item.lastSeen ?? '',
        tags: item.tags ?? [],
        password: item.password ?? ''
      };
    });
  }

  async upsertDevice(id: string, name: string, password: string): Promise<DeviceItem[]> {
    const devices = await this.loadDevices();
    const existing = devices.find((d: DeviceItem) => d.id === id);
    const trimmedName = (name ?? '').trim();
    if (existing) {
      // 传入的是自定义别名时才覆盖；用 ID 当 name 时保留已有别名
      const hasCustomName = existing.name.length > 0 && existing.name !== id;
      if (trimmedName.length > 0 && trimmedName !== id) {
        existing.name = trimmedName;
      } else if (!hasCustomName) {
        existing.name = trimmedName.length > 0 ? trimmedName : id;
      }
      existing.lastSeen = '刚刚';
      if (password.length > 0) {
        existing.password = password;
      }
    } else {
      devices.unshift({
        id,
        name: trimmedName.length > 0 ? trimmedName : id,
        status: 'Unknown',
        lastSeen: '刚刚',
        tags: [],
        password: password
      });
    }
    await this.saveDevices(devices);
    return devices;
  }

  async setDeviceStatus(id: string, status: string, lastSeen: string = ''): Promise<DeviceItem[]> {
    const devices = await this.loadDevices();
    const existing = devices.find((d: DeviceItem) => d.id === id);
    if (existing) {
      existing.status = status;
      if (lastSeen.length > 0) {
        existing.lastSeen = lastSeen;
      } else if (status === 'Online') {
        existing.lastSeen = '当前在线';
      } else if (status === 'Offline') {
        existing.lastSeen = '离线';
      }
      await this.saveDevices(devices);
    }
    return devices;
  }

  async saveDevices(devices: DeviceItem[]): Promise<void> {
    await this.getStore().put(KEY_DEVICES, JSON.stringify(devices));
    await this.getStore().flush();
  }

  async loadPolicy(): Promise<PolicyConfig> {
    const raw = await this.getStore().get(KEY_POLICY, JSON.stringify(DEFAULT_POLICY)) as string;
    return this.safeParse<PolicyConfig>(raw, DEFAULT_POLICY);
  }

  async savePolicy(policy: PolicyConfig): Promise<void> {
    await this.getStore().put(KEY_POLICY, JSON.stringify(policy));
    await this.getStore().flush();
  }

  async loadRelay(): Promise<RelayConfig> {
    const raw = await this.getStore().get(KEY_RELAY, JSON.stringify(DEFAULT_RELAY)) as string;
    return this.safeParse<RelayConfig>(raw, DEFAULT_RELAY);
  }

  async saveRelay(config: RelayConfig): Promise<void> {
    await this.getStore().put(KEY_RELAY, JSON.stringify(config));
    await this.getStore().flush();
  }

  async loadGroups(): Promise<DeviceGroup[]> {
    const raw = await this.getStore().get(KEY_GROUPS, JSON.stringify(DEFAULT_GROUPS)) as string;
    return this.safeParse<DeviceGroup[]>(raw, DEFAULT_GROUPS);
  }

  async saveGroups(groups: DeviceGroup[]): Promise<void> {
    await this.getStore().put(KEY_GROUPS, JSON.stringify(groups));
    await this.getStore().flush();
  }

  async loadSessions(): Promise<RecentSession[]> {
    const raw = await this.getStore().get(KEY_SESSIONS, JSON.stringify(DEFAULT_SESSIONS)) as string;
    return this.safeParse<RecentSession[]>(raw, DEFAULT_SESSIONS);
  }

  async saveSessions(sessions: RecentSession[]): Promise<void> {
    await this.getStore().put(KEY_SESSIONS, JSON.stringify(sessions));
    await this.getStore().flush();
  }

  private getStore(): preferences.Preferences {
    if (!this.prefs) {
      throw new Error('RustDeskStore is not initialized');
    }
    return this.prefs;
  }

  private safeParse<T>(raw: string, fallback: T): T {
    try {
      return JSON.parse(raw) as T;
    } catch {
      return fallback;
    }
  }
}
