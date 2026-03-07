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

  ws.onmessage = (data) => {
    console.log('WebSocket received message:', data.data)
  }
  
  ws.onerror = (err) => {
    console.debug('WebSocket error:', err);
    ws!.close();
  };
}

function sendToMonitor(message: object): boolean {
  if (ws == null)
    return false;
  
  try {
    ws.send(JSON.stringify(message));
  } catch (e) {
    console.error(e);
    return false;
  }
  
  return true;
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
    sendToMonitor(json_download_data);
    
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
    if (!message) return;
    if (message.type === 'openSidebar') 
      browser.sidebarAction.open()
    
    sendToMonitor(message);
  })

}

if (!isFirefoxLike) {
  connectWebSocket();
  browser.downloads.onCreated.addListener(registerDownload);

  browser.runtime.onMessage.addListener((message: any, sender: any) => {
    console.debug(message, sender)
    if (!message) return;
    if (message.type && message.type === 'openSidebar')
      browser.sidebarAction.open()

    sendToMonitor(message);
  })
  
  chrome.action.onClicked.addListener(() => {
    chrome.sidePanel.setPanelBehavior({openPanelOnActionClick: true})
  })
}

// browser.runtime.onMessage.addListener((message: any, sender: any) => {
//   console.debug(message, sender)
  
//   if (!message || message.type !== 'openSidebar') return

//   chrome.sidePanel.setPanelBehavior({openPanelOnActionClick: true})

//   if (!chrome.sidePanel.open) return

//   chrome.tabs.query({active: true, currentWindow: true}, (tabs) => {
//     const activeTabId = tabs && tabs[0] && tabs[0].id
//     if (!activeTabId) return

//     try {
//       chrome.sidePanel.open({tabId: activeTabId})
//     } catch (error) {
//       console.error(error)
//     }
//   })

//   return true;
// })
