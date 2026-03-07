import browser from 'webextension-polyfill';

console.log('Started RustDownloadManager...')
var ws: WebSocket | null = null;

function connectWebSocket() {
  ws = new WebSocket('ws://127.0.0.1:6969');

  ws.onopen = () => {
    console.log('WebSocket connected');
  };

  ws.onclose = () => {
    console.log('WebSocket closed, reconnecting in 5s...');
    setTimeout(connectWebSocket, 5000);
  };
  
  ws.onerror = (err) => {
    console.debug('WebSocket error:', err);
    ws!.close();
  };
}

async function registerDownload(downloadItem: browser.Downloads.DownloadItem) {
  if (ws && ws.readyState === WebSocket.OPEN) {
    const json_download_data = {
      event: 'download_created',
      url: downloadItem.url,
      filename: downloadItem.filename,
      id: downloadItem.id,
      mime: downloadItem.mime,
    };
    console.debug(downloadItem);
    ws.send(JSON.stringify(json_download_data));
    
    browser.downloads.cancel(downloadItem.id);
  }
  
  // browser.downloads.erase({ id: downloadItem.id });
};

const isFirefoxLike =
  import.meta.env.EXTENSION_PUBLIC_BROWSER === 'firefox' ||
  import.meta.env.EXTENSION_PUBLIC_BROWSER === 'gecko-based'

if (isFirefoxLike) {
  connectWebSocket();
  browser.browserAction.onClicked.addListener(() => {
    browser.sidebarAction.open()
  })

  browser.runtime.onMessage.addListener((message: any) => {
    if (!message || message.type !== 'openSidebar') return

    browser.sidebarAction.open()
  })

}

if (!isFirefoxLike) {
  connectWebSocket();
  browser.downloads.onCreated.addListener(registerDownload);
  
  chrome.action.onClicked.addListener(() => {
    chrome.sidePanel.setPanelBehavior({openPanelOnActionClick: true})
  })
}

chrome.runtime.onMessage.addListener((message: any) => {
  if (!message || message.type !== 'openSidebar') return

  chrome.sidePanel.setPanelBehavior({openPanelOnActionClick: true})

  if (!chrome.sidePanel.open) return

  chrome.tabs.query({active: true, currentWindow: true}, (tabs) => {
    const activeTabId = tabs && tabs[0] && tabs[0].id
    if (!activeTabId) return

    try {
      chrome.sidePanel.open({tabId: activeTabId})
    } catch (error) {
      console.error(error)
    }
  })
})
