import router from '@ohos.router';
import type common from '@ohos.app.ability.common';
import { DeviceItem, DeviceGroup, RecentSession, RustDeskStore, isOfficialPlaceholderHost, isPlaceholderKey, isDirectIpId } from '../services/RustDeskStore';

@Entry
@Component
struct Index {
  @State activeTab: number = 0;
  @State devices: DeviceItem[] = [];
  @State sessions: RecentSession[] = [];
  @State quickId: string = '';
  @State quickPassword: string = '';
  @State relayEnabled: boolean = true;
  @State directEnabled: boolean = true;
  @State relayHost: string = '';
  @State apiHost: string = '';
  @State apiKey: string = '';
  @State settingsHint: string = '';
  @State editingServer: boolean = false;
  @State statusProbing: boolean = false;
  @State clipboardSync: boolean = true;
  @State fileTransfer: boolean = true;
  @State loadError: string = '';
  @State editingDeviceId: string = '';
  @State showEditDialog: boolean = false;
  @State showNewConnectDialog: boolean = false;
  @State editDeviceName: string = '';
  @State editDevicePassword: string = '';

  private store: RustDeskStore = new RustDeskStore();
  private storeReady: boolean = false;
  private statusRefreshToken: number = 0;

  aboutToAppear() {
    this.loadData();
  }

  onPageShow() {
    if (this.storeReady && !this.showEditDialog && !this.showNewConnectDialog) {
      this.refreshDeviceStatuses();
    }
  }

  build() {
    Stack() {
      Column() {
        this.header()
        if (this.loadError) {
          Text(this.loadError)
            .fontSize(12)
            .fontColor($r('app.color.status_offline'))
            .padding({ bottom: 6 })
        }
        Tabs({ index: this.activeTab }) {
          TabContent() {
            this.devicesTab()
          }
          .tabBar('设备')
          TabContent() {
            this.settingsTab()
          }
          .tabBar('设置')
        }
        .barPosition(BarPosition.End)
        .onChange((index: number) => {
          this.activeTab = index;
        })
        .layoutWeight(1)
      }
      .padding({ left: 4, right: 4, top: 0 })
      .height('100%')
      .width('100%')

      // 新建连接
      if (this.showNewConnectDialog) {
        Stack({ alignContent: Alignment.Center }) {
          Column()
            .width('100%')
            .height('100%')
            .backgroundColor('rgba(0, 0, 0, 0.55)')
            .onClick(() => {
              this.cancelNewConnect();
            })

          Column({ space: 16 }) {
            Text('新建连接')
              .fontSize(20)
              .fontWeight(FontWeight.Bold)
              .fontColor($r('app.color.text_primary'))
            Text('填设备 ID，或填 IP 直连（如 192.168.1.8，默认端口 21118）')
              .fontSize(13)
              .fontColor($r('app.color.text_muted'))

            this.inputField('设备 ID / IP', this.quickId, (value: string) => {
              this.quickId = value;
            }, InputType.Normal, '例如 123456789 或 192.168.1.8')

            this.inputField('远程密码', this.quickPassword, (value: string) => {
              this.quickPassword = value;
            }, InputType.Password, '对方设置的远程密码')

            Row({ space: 10 }) {
              Button('取消')
                .type(ButtonType.Capsule)
                .layoutWeight(1)
                .height(44)
                .borderRadius(22)
                .fontColor($r('app.color.text_primary'))
                .backgroundColor($r('app.color.chip_background'))
                .onClick(() => {
                  this.cancelNewConnect();
                })
              Button('连接')
                .type(ButtonType.Capsule)
                .layoutWeight(1)
                .height(44)
                .borderRadius(22)
                .fontColor($r('app.color.text_on_accent'))
                .backgroundColor($r('app.color.accent'))
                .onClick(() => {
                  this.openSession();
                })
            }
          }
          .width('88%')
          .padding(22)
          .borderRadius(20)
          .backgroundColor($r('app.color.card_background'))
          .shadow({ radius: 24, color: 'rgba(0,0,0,0.28)', offsetY: 8 })
          .zIndex(2)
          .onClick(() => {})
        }
        .width('100%')
        .height('100%')
        .zIndex(100)
      }

      // 编辑设备对话框：必须用 Stack，否则全屏遮罩会把表单顶出屏幕
      if (this.showEditDialog) {
        Stack({ alignContent: Alignment.Center }) {
          Column()
            .width('100%')
            .height('100%')
            .backgroundColor('rgba(0, 0, 0, 0.55)')
            .onClick(() => {
              this.cancelDeviceEdit();
            })

          Column({ space: 16 }) {
            Text('设备设置')
              .fontSize(20)
              .fontWeight(FontWeight.Bold)
              .fontColor($r('app.color.text_primary'))

            Column({ space: 8 }) {
              Text('设备 ID')
                .fontSize(12)
                .fontColor($r('app.color.text_muted'))
              Text(this.editingDeviceId)
                .fontSize(14)
                .fontColor($r('app.color.text_secondary'))
                .padding(12)
                .borderRadius(12)
                .backgroundColor($r('app.color.input_background'))
                .width('100%')
            }

            Column({ space: 6 }) {
              Text('设备名称')
                .fontSize(12)
                .fontColor($r('app.color.text_muted'))
              TextInput({ text: this.editDeviceName, placeholder: '给设备起个名字' })
                .maxLength(64)
                .fontColor($r('app.color.text_primary'))
                .placeholderColor($r('app.color.text_muted'))
                .caretColor($r('app.color.accent'))
                .onChange((value: string) => {
                  this.editDeviceName = value;
                })
                .height(44)
                .width('100%')
                .padding({ left: 12, right: 12 })
                .borderRadius(12)
                .backgroundColor($r('app.color.input_background'))
            }

            this.inputField('远程密码', this.editDevicePassword, (value: string) => {
              this.editDevicePassword = value;
            }, InputType.Password, '保存后点连接可直接登录')

            Row({ space: 10 }) {
              Button('取消')
                .type(ButtonType.Capsule)
                .layoutWeight(1)
                .height(44)
                .borderRadius(22)
                .fontColor($r('app.color.text_primary'))
                .backgroundColor($r('app.color.chip_background'))
                .onClick(() => {
                  this.cancelDeviceEdit();
                })
              Button('保存')
                .type(ButtonType.Capsule)
                .layoutWeight(1)
                .height(44)
                .borderRadius(22)
                .fontColor($r('app.color.text_on_accent'))
                .backgroundColor($r('app.color.accent'))
                .onClick(() => {
                  this.saveDeviceEdit();
                })
            }

            Button('删除此设备')
              .type(ButtonType.Normal)
              .width('100%')
              .height(40)
              .borderRadius(12)
              .fontSize(14)
              .fontColor('#E53935')
              .backgroundColor(Color.Transparent)
              .onClick(() => {
                const device = this.devices.find((d: DeviceItem) => d.id === this.editingDeviceId);
                if (device) {
                  this.deleteDevice(device);
                }
                this.cancelDeviceEdit();
              })
          }
          .width('88%')
          .padding(22)
          .borderRadius(20)
          .backgroundColor($r('app.color.card_background'))
          .shadow({ radius: 24, color: 'rgba(0,0,0,0.28)', offsetY: 8 })
          .zIndex(2)
          .onClick(() => {})
        }
        .width('100%')
        .height('100%')
        .zIndex(100)
      }
    }
    .backgroundColor($r('app.color.app_background_top'))
    .height('100%')
    .width('100%')
  }

  @Builder
  header() {
    Row() {
      Column({ space: 4 }) {
        Text('远程桌面')
          .fontSize(26)
          .fontWeight(FontWeight.Bold)
          .fontColor($r('app.color.text_primary'))
        Text('控制你的电脑')
          .fontSize(13)
          .fontColor($r('app.color.text_secondary'))
      }
      .alignItems(HorizontalAlign.Start)
      .layoutWeight(1)
    }
    .width('100%')
    .padding({ left: 8, right: 8, top: 8, bottom: 16 })
  }

  @Builder
  devicesTab() {
    Scroll() {
      Column({ space: 14 }) {
        Row() {
          Column({ space: 2 }) {
            Text('我的设备')
              .fontSize(18)
              .fontWeight(FontWeight.Medium)
              .fontColor($r('app.color.text_primary'))
            Text(this.devices.length > 0 ? `${this.devices.length} 台设备` : '还没有保存的设备')
              .fontSize(12)
              .fontColor($r('app.color.text_muted'))
          }
          .alignItems(HorizontalAlign.Start)
          .layoutWeight(1)

          Button('新建')
            .type(ButtonType.Capsule)
            .fontSize(14)
            .fontWeight(FontWeight.Medium)
            .height(40)
            .padding({ left: 18, right: 18 })
            .borderRadius(20)
            .fontColor($r('app.color.text_on_accent'))
            .backgroundColor($r('app.color.accent'))
            .onClick(() => {
              this.openNewConnect();
            })
        }
        .width('100%')
        .alignItems(VerticalAlign.Center)
        .padding({ left: 4, right: 4 })

        if (this.devices.length === 0) {
          Column({ space: 12 }) {
            Text('开始远程控制')
              .fontSize(16)
              .fontWeight(FontWeight.Medium)
              .fontColor($r('app.color.text_primary'))
            Text('点右上角「新建」，输入设备 ID 或 IP（如 192.168.1.8）即可连接')
              .fontSize(13)
              .fontColor($r('app.color.text_muted'))
              .textAlign(TextAlign.Center)
            Button('新建连接')
              .type(ButtonType.Capsule)
              .height(44)
              .padding({ left: 28, right: 28 })
              .borderRadius(22)
              .fontColor($r('app.color.text_on_accent'))
              .backgroundColor($r('app.color.accent'))
              .onClick(() => {
                this.openNewConnect();
              })
          }
          .width('100%')
          .padding({ top: 36, bottom: 36, left: 20, right: 20 })
          .borderRadius(20)
          .backgroundColor($r('app.color.card_background'))
        } else {
          Column({ space: 12 }) {
            ForEach(this.devices, (device: DeviceItem) => {
              Column({ space: 14 }) {
                Row({ space: 12 }) {
                  Column() {
                    Text(this.deviceInitial(device.name))
                      .fontSize(18)
                      .fontWeight(FontWeight.Bold)
                      .fontColor($r('app.color.text_on_accent'))
                  }
                  .width(44)
                  .height(44)
                  .borderRadius(22)
                  .justifyContent(FlexAlign.Center)
                  .backgroundColor($r('app.color.accent'))

                  Column({ space: 4 }) {
                    Text(device.name)
                      .fontSize(16)
                      .fontWeight(FontWeight.Medium)
                      .fontColor($r('app.color.text_primary'))
                      .maxLines(1)
                      .textOverflow({ overflow: TextOverflow.Ellipsis })
                    Text(device.id)
                      .fontSize(12)
                      .fontColor($r('app.color.text_secondary'))
                    Text((device.password ?? '').length > 0 ? '密码已保存 · 可一键连接' : '未保存密码')
                      .fontSize(11)
                      .fontColor((device.password ?? '').length > 0 ?
                        $r('app.color.status_online') : $r('app.color.text_muted'))
                  }
                  .layoutWeight(1)
                  .alignItems(HorizontalAlign.Start)

                  Text(this.getStatusText(device.status))
                    .fontSize(11)
                    .fontWeight(FontWeight.Medium)
                    .fontColor(this.statusColor(device.status))
                    .padding({ left: 10, right: 10, top: 4, bottom: 4 })
                    .borderRadius(10)
                    .backgroundColor($r('app.color.chip_background'))
                }
                .width('100%')
                .alignItems(VerticalAlign.Center)

                Row({ space: 10 }) {
                  Button('连接')
                    .type(ButtonType.Capsule)
                    .fontSize(14)
                    .fontWeight(FontWeight.Medium)
                    .height(40)
                    .borderRadius(20)
                    .layoutWeight(1.4)
                    .fontColor($r('app.color.text_on_accent'))
                    .backgroundColor($r('app.color.accent'))
                    .onClick(() => {
                      this.openDeviceSession(device);
                    })
                  Button('设置')
                    .type(ButtonType.Capsule)
                    .fontSize(14)
                    .height(40)
                    .borderRadius(20)
                    .layoutWeight(1)
                    .fontColor($r('app.color.text_primary'))
                    .backgroundColor($r('app.color.chip_background'))
                    .onClick(() => {
                      this.openDeviceEdit(device);
                    })
                }
                .width('100%')
              }
              .padding(16)
              .borderRadius(18)
              .backgroundColor($r('app.color.card_background'))
              .shadow({ radius: 10, color: 'rgba(0,0,0,0.08)', offsetY: 2 })
            }, (device: DeviceItem) => device.id)
          }
        }
      }
      .width('100%')
      .padding({ left: 8, right: 8, bottom: 24 })
    }
    .width('100%')
    .height('100%')
    .scrollBar(BarState.Off)
  }

  @Builder
  settingsTab() {
    Scroll() {
      Column({ space: 12 }) {
        Column({ space: 10 }) {
          Text('网络设置')
            .fontSize(16)
            .fontWeight(FontWeight.Medium)
            .fontColor($r('app.color.text_primary'))
          this.toggleRow('使用中继服务器', this.relayEnabled, (value: boolean) => {
            this.relayEnabled = value;
            this.saveRelayConfig();
          })
          this.toggleRow('允许直连', this.directEnabled, (value: boolean) => {
            this.directEnabled = value;
            this.saveRelayConfig();
          })
        }
        .padding(16)
        .borderRadius(18)
        .backgroundColor($r('app.color.card_background'))

        // 功能设置
        Column({ space: 10 }) {
          Text('功能设置')
            .fontSize(16)
            .fontWeight(FontWeight.Medium)
            .fontColor($r('app.color.text_primary'))
          this.toggleRow('剪贴板同步', this.clipboardSync, (value: boolean) => {
            this.clipboardSync = value;
            this.saveSettings();
          })
          this.toggleRow('文件传输', this.fileTransfer, (value: boolean) => {
            this.fileTransfer = value;
            this.saveSettings();
          })
        }
        .padding(16)
        .borderRadius(18)
        .backgroundColor($r('app.color.card_background'))

        // 服务器设置：默认折叠，点编辑才展开
        Column({ space: 10 }) {
          Row() {
            Text('服务器设置')
              .fontSize(16)
              .fontWeight(FontWeight.Medium)
              .fontColor($r('app.color.text_primary'))
              .layoutWeight(1)
            Button(this.editingServer ? '收起' : '编辑')
              .type(ButtonType.Normal)
              .fontSize(13)
              .height(32)
              .padding({ left: 14, right: 14 })
              .borderRadius(16)
              .fontColor($r('app.color.text_primary'))
              .backgroundColor($r('app.color.chip_background'))
              .onClick(() => {
                this.editingServer = !this.editingServer;
                if (!this.editingServer) {
                  this.settingsHint = '';
                }
              })
          }
          .width('100%')
          .alignItems(VerticalAlign.Center)

          if (!this.editingServer) {
            Text(this.serverSummary())
              .fontSize(13)
              .fontColor($r('app.color.text_secondary'))
              .width('100%')
          } else {
            Text('填写自建 hbbs / hbbr 地址与密钥。点收起后地址不会显示在设置首页。')
              .fontSize(12)
              .fontColor($r('app.color.text_muted'))
            this.inputField('中继服务器 hbbr', this.relayHost, (value: string) => {
              this.relayHost = value;
            }, InputType.Normal, '例如 192.168.1.10:21117')
            this.inputField('ID 服务器 hbbs', this.apiHost, (value: string) => {
              this.apiHost = value;
            }, InputType.Normal, '例如 192.168.1.10:21116')
            this.inputField('密钥 id_ed25519.pub', this.apiKey, (value: string) => {
              this.apiKey = value;
            }, InputType.Normal, '粘贴公钥内容')
            if (this.settingsHint.length > 0) {
              Text(this.settingsHint)
                .fontSize(12)
                .fontColor($r('app.color.text_secondary'))
            }
            Button('保存配置')
              .width('100%')
              .type(ButtonType.Capsule)
              .height(44)
              .borderRadius(22)
              .fontColor($r('app.color.text_on_accent'))
              .backgroundColor($r('app.color.accent'))
              .onClick(() => {
                this.saveRelayConfig();
              })
          }
        }
        .padding(16)
        .borderRadius(18)
        .backgroundColor($r('app.color.card_background'))
      }
      .width('100%')
      .padding({ left: 8, right: 8, bottom: 24 })
    }
    .width('100%')
    .height('100%')
    .scrollBar(BarState.Off)
  }

  @Builder
  inputField(label: string, value: string, onChange: (value: string) => void, type: InputType = InputType.Normal, placeholder: string = '') {
    Column({ space: 6 }) {
      Text(label)
        .fontSize(12)
        .fontColor($r('app.color.text_muted'))
      TextInput({ text: value, placeholder: placeholder.length > 0 ? placeholder : `请输入${label}` })
        .type(type)
        .maxLength(256)
        .fontColor($r('app.color.text_primary'))
        .placeholderColor($r('app.color.text_muted'))
        .caretColor($r('app.color.accent'))
        .onChange(onChange)
        .height(44)
        .width('100%')
        .padding({ left: 12, right: 12 })
        .borderRadius(12)
        .backgroundColor($r('app.color.input_background'))
    }
  }

  @Builder
  toggleRow(label: string, value: boolean, onChange: (value: boolean) => void) {
    Row() {
      Text(label)
        .fontSize(14)
        .fontColor($r('app.color.text_primary'))
        .layoutWeight(1)
      Toggle({ type: ToggleType.Switch, isOn: value })
        .onChange(onChange)
    }
    .alignItems(VerticalAlign.Center)
  }

  private async loadData(): Promise<void> {
    try {
      const context = getContext(this) as common.UIAbilityContext;
      await this.store.init(context);
      const devices = await this.store.loadDevices();
      const relay = await this.store.loadRelay();
      this.devices = devices;
      this.relayEnabled = relay.relayEnabled;
      this.directEnabled = relay.directEnabled;
      this.relayHost = isOfficialPlaceholderHost(relay.relayHost) ? '' : relay.relayHost;
      this.apiHost = isOfficialPlaceholderHost(relay.apiHost) ? '' : relay.apiHost;
      this.apiKey = isPlaceholderKey(relay.key) ? '' : relay.key;
      this.storeReady = true;
      this.applyNativeConfig();
      this.loadError = '';
      this.refreshDeviceStatuses();
    } catch (error) {
      this.loadError = `加载数据失败: ${error.message ?? error}`;
    }
  }

  private async saveSettings(): Promise<void> {
    if (!this.storeReady) {
      return;
    }
    await this.store.savePolicy({
      autoAccept: false,
      clipboardSync: this.clipboardSync,
      fileTransfer: this.fileTransfer,
      viewOnlyDefault: false
    });
  }

  private async saveRelayConfig(): Promise<void> {
    try {
      if (!this.storeReady) {
        const context = getContext(this) as common.UIAbilityContext;
        await this.store.init(context);
        this.storeReady = true;
      }
      const idServer = this.apiHost.trim();
      const relayHost = this.relayHost.trim();
      const key = this.apiKey.replace(/\s/g, '');
      if (isOfficialPlaceholderHost(idServer)) {
        this.settingsHint = '没读到 ID 服务器。请重新输入 hbbs 地址后再点保存。';
        return;
      }
      await this.store.saveRelay({
        relayEnabled: this.relayEnabled,
        directEnabled: this.directEnabled,
        selfHostEnabled: true,
        relayHost: relayHost,
        apiHost: idServer,
        key: key
      });
      this.relayHost = relayHost;
      this.apiHost = idServer;
      this.apiKey = key;
      if (key.length === 0) {
        this.settingsHint = '已保存。密钥还是空的，连接时 hbbs 会拒绝。';
      } else if (key.length < 40 || key.length > 100) {
        this.settingsHint = `已保存，密钥长度 ${key.length}。公钥一般是 44 个字符。`;
      } else {
        this.settingsHint = `已保存，密钥长度 ${key.length}`;
      }
      this.editingServer = false;
      this.refreshDeviceStatuses();
    } catch (error) {
      this.settingsHint = `保存失败: ${error instanceof Error ? error.message : String(error)}`;
    }
  }

  private serverSummary(): string {
    const idServer = this.apiHost.trim();
    if (isOfficialPlaceholderHost(idServer)) {
      return '未配置，点编辑填写服务器';
    }
    const keyOk = this.apiKey.replace(/\s/g, '').length > 0;
    const relayOk = this.relayHost.trim().length > 0;
    if (keyOk && relayOk) {
      return '已配置 · 点编辑可修改';
    }
    if (keyOk) {
      return '已配置 ID 服务器 · 密钥已填';
    }
    return '已配置 ID 服务器 · 密钥未填';
  }

  private async applyNativeConfig(): Promise<void> {
    console.info('[Index] skip native config until session connect');
  }

  private async refreshDeviceStatuses(): Promise<void> {
    if (!this.storeReady) {
      return;
    }
    // 会话进行中不要探测，避免抢连接/打日志干扰画面
    try {
      const keepAliveMod = await import('../services/SessionKeepAlive');
      if (keepAliveMod.SessionKeepAlive.isActive()) {
        return;
      }
    } catch (_e) {
      // ignore
    }
    const idServer = this.apiHost.trim();
    const hasServer = !isOfficialPlaceholderHost(idServer);
    const hasIpDevice = this.devices.some((d: DeviceItem) => isDirectIpId(d.id));
    if (!hasServer && !hasIpDevice) {
      this.markUnknownDevicesOffline('未配置服务器');
      return;
    }
    const token = ++this.statusRefreshToken;
    this.statusProbing = true;
    try {
      const nativeMod = await import('../native/HarmonyDeskNative');
      const native = await nativeMod.getNativeModule();
      if (!native) {
        this.markUnknownDevicesOffline('Native 未就绪');
        return;
      }
      if (hasServer) {
        native.setServerConfig(
          idServer,
          this.relayHost.trim(),
          !this.directEnabled,
          this.apiKey.replace(/\s/g, '')
        );
        await new Promise<void>((resolve) => setTimeout(resolve, 600));
      }
      const ids: string[] = [];
      for (let i = 0; i < this.devices.length; i++) {
        ids.push(this.devices[i].id);
      }
      for (let i = 0; i < ids.length; i++) {
        if (token !== this.statusRefreshToken) {
          return;
        }
        const id = ids[i];
        if (id.startsWith('DEMO-')) {
          this.applyDeviceStatus(id, 'Online', '当前在线');
          continue;
        }
        if (!isDirectIpId(id) && !hasServer) {
          this.applyDeviceStatus(id, 'Unknown', '未配置服务器');
          continue;
        }
        let rc = native.checkPeer(id);
        for (let retry = 0; rc === -1 && retry < 30 && token === this.statusRefreshToken; retry++) {
          await new Promise<void>((resolve) => setTimeout(resolve, 100));
          if (hasServer) {
            native.setServerConfig(
              idServer,
              this.relayHost.trim(),
              !this.directEnabled,
              this.apiKey.replace(/\s/g, '')
            );
          }
          rc = native.checkPeer(id);
        }
        if (token !== this.statusRefreshToken) {
          return;
        }
        if (rc === 2) {
          this.applyDeviceStatus(id, 'Online', '当前在线');
          continue;
        }
        if (rc === -2) {
          this.applyDeviceStatus(id, 'Unknown', '服务器未就绪');
          continue;
        }
        if (rc !== 0) {
          this.applyDeviceStatus(id, 'Unknown', '检测失败');
          continue;
        }
        let result = '';
        for (let t = 0; t < 150; t++) {
          if (token !== this.statusRefreshToken) {
            return;
          }
          await new Promise<void>((resolve) => setTimeout(resolve, 100));
          result = native.getCheckResult();
          if (result.length > 0 && result !== 'PENDING') {
            break;
          }
        }
        if (result === 'ONLINE') {
          this.applyDeviceStatus(id, 'Online', isDirectIpId(id) ? 'IP可达' : '当前在线');
        } else if (result === 'OFFLINE') {
          this.applyDeviceStatus(id, 'Offline', isDirectIpId(id) ? '端口不通' : '离线');
        } else if (result === 'NOT_EXIST') {
          this.applyDeviceStatus(id, 'Offline', 'ID不存在');
        } else if (result === 'LICENSE_MISMATCH' || result === 'LICENSE_OVERUSE') {
          this.applyDeviceStatus(id, 'Unknown', '密钥异常');
        } else if (result.startsWith('ERROR:')) {
          // Network/protocol glitches must not look like the PC is offline.
          this.applyDeviceStatus(id, 'Unknown', '检测失败');
        } else {
          this.applyDeviceStatus(id, 'Unknown', '检测失败');
        }
      }
      if (token === this.statusRefreshToken) {
        await this.store.saveDevices(this.devices);
      }
    } catch (error) {
      console.error('[Index] refreshDeviceStatuses failed:', error);
      this.markUnknownDevicesOffline('检测异常');
    } finally {
      if (token === this.statusRefreshToken) {
        this.statusProbing = false;
      }
    }
  }

  private applyDeviceStatus(id: string, status: string, lastSeen: string): void {
    const next: DeviceItem[] = [];
    for (let i = 0; i < this.devices.length; i++) {
      const d = this.devices[i];
      if (d.id === id) {
        const item: DeviceItem = {
          id: d.id,
          name: d.name,
          status: status,
          lastSeen: lastSeen,
          tags: d.tags.slice(),
          password: d.password
        };
        next.push(item);
      } else {
        const copy: DeviceItem = {
          id: d.id,
          name: d.name,
          status: d.status,
          lastSeen: d.lastSeen,
          tags: d.tags.slice(),
          password: d.password
        };
        next.push(copy);
      }
    }
    this.devices = next;
  }

  private markUnknownDevicesOffline(reason: string): void {
    let changed = false;
    const next: DeviceItem[] = [];
    for (let i = 0; i < this.devices.length; i++) {
      const d = this.devices[i];
      if (d.status === 'Unknown' || d.status.length === 0) {
        changed = true;
        const item: DeviceItem = {
          id: d.id,
          name: d.name,
          status: 'Unknown',
          lastSeen: reason,
          tags: d.tags.slice(),
          password: d.password
        };
        next.push(item);
      } else {
        next.push(d);
      }
    }
    if (changed) {
      this.devices = next;
      if (this.storeReady) {
        this.store.saveDevices(this.devices).catch(() => {});
      }
    }
  }

  private statusColor(status: string): ResourceColor {
    if (status === 'Online') {
      return $r('app.color.status_online');
    }
    if (status === 'Sleeping' || status === 'Unknown') {
      return $r('app.color.status_idle');
    }
    return $r('app.color.status_offline');
  }

  private getStatusText(status: string): string {
    switch (status) {
      case 'Online': return '在线';
      case 'Sleeping': return '休眠';
      case 'Offline': return '离线';
      case 'Unknown': return this.statusProbing ? '检测中' : '未知';
      default: return status.length > 0 ? status : (this.statusProbing ? '检测中' : '未知');
    }
  }

  private async saveDeviceToList(id: string, name: string, password: string = ''): Promise<void> {
    if (!this.storeReady) {
      try {
        const context = getContext(this) as common.UIAbilityContext;
        await this.store.init(context);
        this.storeReady = true;
      } catch (_e) {
        return;
      }
    }
    const displayName = (name && name !== id) ? name : (isDirectIpId(id) ? `IP ${id}` : id);
    const existing = this.devices.findIndex((d: DeviceItem) => d.id === id);
    if (existing >= 0) {
      const next: DeviceItem[] = this.devices.slice();
      const old = next[existing];
      next[existing] = {
        id: old.id,
        name: displayName,
        status: old.status,
        lastSeen: old.lastSeen,
        tags: old.tags.slice(),
        password: password.length > 0 ? password : old.password
      };
      this.devices = next;
    } else {
      const item: DeviceItem = {
        id: id,
        name: displayName,
        status: 'Unknown',
        lastSeen: isDirectIpId(id) ? 'IP直连' : '刚添加',
        tags: [],
        password: password
      };
      this.devices = [item].concat(this.devices);
    }
    await this.store.saveDevices(this.devices);
  }

  private openDeviceEdit(device: DeviceItem): void {
    this.statusRefreshToken++;
    this.statusProbing = false;
    this.editingDeviceId = device.id;
    this.editDeviceName = device.name;
    this.editDevicePassword = device.password ?? '';
    this.showEditDialog = true;
    this.loadError = '';
  }

  private openNewConnect(): void {
    this.quickId = '';
    this.quickPassword = '';
    this.loadError = '';
    this.showNewConnectDialog = true;
  }

  private cancelNewConnect(): void {
    this.showNewConnectDialog = false;
  }

  private deviceInitial(name: string): string {
    const value = name.trim();
    if (value.length === 0) {
      return '#';
    }
    return value.substring(0, 1).toUpperCase();
  }

  private openSession(): void {
    console.info('[Index] openSession called, quickId:', this.quickId);

    if (!this.quickId || this.quickId.trim().length === 0) {
      this.loadError = '请输入设备 ID 或 IP';
      console.warn('[Index] quickId is empty');
      return;
    }

    const id = this.quickId.trim();
    const password = this.quickPassword || '';
    this.showNewConnectDialog = false;
    this.saveDeviceToList(id, id, password);

    interface SessionParams {
      id: string;
      name: string;
      status: string;
      password: string;
      viewOnly: string;
    }

    const params: SessionParams = {
      id: id,
      name: id,
      status: 'Unknown',
      password: password,
      viewOnly: '0'
    };

    console.info('[Index] Pushing to Session with id:', params.id);

    router.pushUrl({
      url: 'pages/Session',
      params: params
    }).catch((error: Error) => {
      console.error('[Index] pushUrl failed:', error);
      this.loadError = `跳转失败: ${error.message || String(error)}`;
    });
  }

  private openDeviceSession(device: DeviceItem): void {
    const password = (device.password ?? '').trim();
    if (password.length === 0 && !device.id.startsWith('DEMO') && !device.id.startsWith('TEST')) {
      this.openDeviceEdit(device);
      this.loadError = '请先填写并保存远程密码，再点连接';
      return;
    }

    interface SessionParams {
      id: string;
      name: string;
      status: string;
      password: string;
      viewOnly: string;
    }

    const params: SessionParams = {
      id: device.id,
      name: device.name,
      status: device.status,
      password: password,
      viewOnly: '0'
    };

    router.pushUrl({
      url: 'pages/Session',
      params: params
    }).catch((error: Error) => {
      console.error('[Index] pushUrl failed:', error);
      this.loadError = `跳转失败: ${error.message || String(error)}`;
    });
  }

  private async saveDeviceEdit(): Promise<void> {
    if (this.editingDeviceId.length === 0) {
      return;
    }
    // 取消进行中的状态刷新，避免浅拷贝把刚改的名称盖掉
    this.statusRefreshToken++;
    this.statusProbing = false;
    const editId = this.editingDeviceId;
    const newName = this.editDeviceName.trim();
    const newPassword = this.editDevicePassword;
    try {
      if (!this.storeReady) {
        const context = getContext(this) as common.UIAbilityContext;
        await this.store.init(context);
        this.storeReady = true;
      }
      const fromStore = await this.store.loadDevices();
      const next: DeviceItem[] = [];
      let found = false;
      for (let i = 0; i < fromStore.length; i++) {
        const d = fromStore[i];
        if (d.id === editId) {
          found = true;
          const item: DeviceItem = {
            id: d.id,
            name: newName.length > 0 ? newName : d.id,
            status: d.status,
            lastSeen: d.lastSeen,
            tags: d.tags.slice(),
            password: newPassword
          };
          next.push(item);
        } else {
          next.push(d);
        }
      }
      if (!found) {
        this.cancelDeviceEdit();
        return;
      }
      await this.store.saveDevices(next);
      this.devices = next;
      this.loadError = '';
    } catch (error) {
      this.loadError = `保存失败: ${error instanceof Error ? error.message : String(error)}`;
    }
    this.cancelDeviceEdit();
  }

  private cancelDeviceEdit(): void {
    this.showEditDialog = false;
    this.editingDeviceId = '';
    this.editDeviceName = '';
    this.editDevicePassword = '';
  }

  private async deleteDevice(device: DeviceItem): Promise<void> {
    this.devices = this.devices.filter(d => d.id !== device.id);
    if (this.storeReady) {
      await this.store.saveDevices(this.devices);
    }
  }
}
