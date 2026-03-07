import { useState, useEffect } from 'react';
import iconUrl from '../images/icon.png';
import "./styles.css"

// Type for configuration sent to service worker
interface DownloadConfig {
  filenamePattern: string;
  downloadFolder: string;
}

const SidebarApp = () => {
  const [filename, setFilename] = useState('');
  const [folder, setFolder] = useState('');
  const [status, setStatus] = useState('');

  // Load current config from service worker on mount
  useEffect(() => {
    const msg = browser.runtime.sendMessage({ event: 'GET_CONFIG' });    
    msg.then((v) => {
      console.debug(v);
    })
  }, []);

  const saveConfig = () => {
    const config: DownloadConfig = {
      filenamePattern: filename,
      downloadFolder: folder,
    };

    if (browser?.runtime?.sendMessage) {
      browser.runtime.sendMessage(
        { event: 'SET_CONFIG', config }
      ).then((v) => {
        console.log('Config saved.', v);        
      });
    }
  };

  return (
    <div className="sidebar_app">
      <img className="sidebar_logo" src={iconUrl} alt="React logo" />
      <h1 className="sidebar_title">Download Manager</h1>

      <div className="form-group">
        <label htmlFor="filename">Filename pattern:</label>
        <input
          id="filename"
          type="text"
          value={filename}
          onChange={(e) => setFilename(e.target.value)}
          placeholder="e.g., myfile-{date}.txt"
        />
      </div>

      <div className="form-group">
        <label htmlFor="folder">Downloads folder:</label>
        <input
          id="folder"
          type="text"
          value={folder}
          onChange={(e) => setFolder(e.target.value)}
          placeholder="e.g., /Users/name/Downloads"
        />
      </div>

      <button onClick={saveConfig}>Save Settings</button>
      {status && <p className="status">{status}</p>}

      <p className="sidebar_description">
        Settings are sent to the service worker, which manages the WebSocket
        connection. See{' '}
        <a
          href="https://extension.js.org"
          target="_blank"
          rel="noopener noreferrer"
        >
          docs
        </a>
        .
      </p>
    </div>
  );
};

export default SidebarApp;
