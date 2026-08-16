import router from '@ohos.router';

@Entry
@Component
struct DeviceDetail {
  @State allowKeyboard: boolean = true;
  @State allowClipboard: boolean = true;
  @State allowFile: boolean = true;
  @State autoReconnect: boolean = true;

  private getParam(key: string): string {
    return '';
  }

  build() {
    Stack() {
      Column({ space: 12 }) {
        this.header()
        this.deviceSummary()
        this.actions()
        this.permissions()
        this.networkInfo()
      }
      .padding({ left: 16, right: 16, top: 16 })
      .height('100%')
      .width('100%')
    }
    .backgroundColor($r('app.color.app_background_top'))
    .height('100%')
    .width('100%')
  }

  @Builder
  header() {
    Row({ space: 12 }) {
      Button('Back')
        .type(ButtonType.Normal)
        .onClick(() => {
          router.back();
        })
      Column({ space: 2 }) {
        Text('Device Detail')
          .fontSize(20)
          .fontWeight(FontWeight.Bold)
          .fontColor($r('app.color.text_primary'))
        Text('Manage access rules & connection')
          .fontSize(12)
          .fontColor($r('app.color.text_secondary'))
      }
    }
    .alignItems(VerticalAlign.Center)
  }

  @Builder
  deviceSummary() {
    Column({ space: 6 }) {
      Text(this.getParam('name') || 'Unknown Device')
        .fontSize(18)
        .fontWeight(FontWeight.Medium)
        .fontColor($r('app.color.text_primary'))
      Text(`${this.getParam('id')} · ${this.getParam('status')}`)
        .fontSize(12)
        .fontColor($r('app.color.text_secondary'))
      Text(`Last seen: ${this.getParam('lastSeen') || 'Unknown'}`)
        .fontSize(11)
        .fontColor($r('app.color.text_muted'))
    }
    .padding(14)
    .borderRadius(18)
    .backgroundColor($r('app.color.card_background'))
  }

  @Builder
  actions() {
    Column({ space: 10 }) {
      Text('Quick Actions')
        .fontSize(16)
        .fontWeight(FontWeight.Medium)
        .fontColor($r('app.color.text_primary'))
      Row({ space: 8 }) {
        Button('Start Session')
          .type(ButtonType.Capsule)
          .fontColor($r('app.color.text_on_accent'))
          .backgroundColor($r('app.color.accent'))
          .onClick(() => {
            router.pushUrl({
              url: 'pages/Session',
              params: {
                id: this.getParam('id'),
                name: this.getParam('name'),
                status: this.getParam('status')
              }
            });
          })
        Button('Wake')
          .type(ButtonType.Normal)
        Button('Reboot')
          .type(ButtonType.Normal)
      }
    }
    .padding(14)
    .borderRadius(18)
    .backgroundColor($r('app.color.card_background'))
  }

  @Builder
  permissions() {
    Column({ space: 10 }) {
      Text('Permissions')
        .fontSize(16)
        .fontWeight(FontWeight.Medium)
        .fontColor($r('app.color.text_primary'))
      this.toggleRow('Allow keyboard & mouse', this.allowKeyboard, (value: boolean) => {
        this.allowKeyboard = value;
      })
      this.toggleRow('Sync clipboard', this.allowClipboard, (value: boolean) => {
        this.allowClipboard = value;
      })
      this.toggleRow('Allow file transfer', this.allowFile, (value: boolean) => {
        this.allowFile = value;
      })
      this.toggleRow('Auto reconnect', this.autoReconnect, (value: boolean) => {
        this.autoReconnect = value;
      })
    }
    .padding(14)
    .borderRadius(18)
    .backgroundColor($r('app.color.card_background'))
  }

  @Builder
  networkInfo() {
    Column({ space: 8 }) {
      Text('Network')
        .fontSize(16)
        .fontWeight(FontWeight.Medium)
        .fontColor($r('app.color.text_primary'))
      Row() {
        Column({ space: 2 }) {
          Text('Connection')
            .fontSize(12)
            .fontColor($r('app.color.text_muted'))
          Text('Relay · 1080p · 24 fps')
            .fontSize(13)
            .fontColor($r('app.color.text_primary'))
        }
        .layoutWeight(1)
        Button('Optimize')
          .type(ButtonType.Normal)
      }
      Row() {
        Column({ space: 2 }) {
          Text('Notes')
            .fontSize(12)
            .fontColor($r('app.color.text_muted'))
          Text('Trusted device · Office group')
            .fontSize(13)
            .fontColor($r('app.color.text_secondary'))
        }
      }
    }
    .padding(14)
    .borderRadius(18)
    .backgroundColor($r('app.color.card_background'))
  }

  @Builder
  toggleRow(label: string, value: boolean, onChange: (value: boolean) => void) {
    Row() {
      Text(label)
        .fontSize(13)
        .fontColor($r('app.color.text_primary'))
        .layoutWeight(1)
      Toggle({ type: ToggleType.Switch, isOn: value })
        .onChange(onChange)
    }
    .alignItems(VerticalAlign.Center)
  }
}
