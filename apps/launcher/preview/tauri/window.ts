const noop = async () => {}
export function getCurrentWindow() {
  return {
    minimize: noop,
    toggleMaximize: noop,
    maximize: noop,
    unmaximize: noop,
    close: noop,
    hide: noop,
    show: noop,
    setFocus: noop,
    isMaximized: async () => false,
    onResized: async () => () => {},
    onFocusChanged: async () => () => {},
    listen: async () => () => {},
  }
}
