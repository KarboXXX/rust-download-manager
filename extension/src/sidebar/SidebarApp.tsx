import { useState, useEffect } from "react";
import logo from "../images/rust.png";
import "./styles.css";
import { isFirefoxLike } from "../background.ts";

// Type for configuration sent to service worker
interface DownloadConfig {
    total_threads: number;
    download_threads_max: number;
    download_threads_min: number;
    download: string;
}

const SidebarApp = () => {
    const [total_threads, setTotal] = useState(0);
    const [minTh, setMinTh] = useState(0);
    const [maxTh, setMaxTh] = useState(0);
    const [folder, setFolder] = useState("");
    const [statusText, setStatusText] = useState("");

    function statusLabel() {
        useEffect(() => {
            setTimeout(() => {
                setStatusText("");
            }, 5000);
        }, [statusText]);

        return <div className="status">{statusText}</div>;
    }

    function setConfig(config: DownloadConfig) {
        setFolder(config.download);
        setMaxTh(config.download_threads_max);
        setMinTh(config.download_threads_min);
        setTotal(config.total_threads);
        console.debug("config set");
    }

    async function getDefaultConfig(): Promise<DownloadConfig> {
        let msg;
        if (isFirefoxLike)
            msg = await browser.runtime.sendMessage({
                event: "GET_DEFAULT_CONFIG",
            });
        else
            msg = await chrome.runtime.sendMessage({
                event: "GET_DEFAULT_CONFIG",
            });

        return msg as DownloadConfig;
    }

    function handleStatusText(
        message: any,
        handler: any,
        sendResponse: any,
    ): any {
        try {
            if (message == "Saved") setStatusText("Saved settings.");
            let msg_obj = JSON.parse(message);
            if (msg_obj.event == "GET_DEFAULT_CONFIG") {
                setConfig(msg_obj);
            }
        } catch (e) {
            console.debug(message);
        }
        sendResponse({ message });
        return true;
    }

    if (isFirefoxLike) browser.runtime.onMessage.addListener(handleStatusText);
    else chrome.runtime.onMessage.addListener(handleStatusText);

    // Load current config from service worker on mount
    useEffect(() => {
        function handleStorageConfig(config) {
            console.debug("config -> ", config.config);
            if (!config || !config.config) {
                console.debug("empty config");
                getDefaultConfig()
                    .then((def_config) => {
                        if (!def_config)
                            return setStatusText(
                                "Error on getting service information.",
                            );

                        console.debug("config passed -> ", def_config);
                        setConfig(def_config);
                    })
                    .catch((e) => {
                        console.error(e);
                        const settings = config.config as DownloadConfig;
                        setConfig(settings);
                        saveConfig();
                    });
            }

            let setting = config.config as DownloadConfig;
            if (setting.download_threads_max <= 0)
                setting.download_threads_max = 1;

            if (setting.download_threads_min <= 0)
                setting.download_threads_min = 1;

            setConfig(setting);
        }

        function handleStorageConfigError(e) {
            getDefaultConfig().then((def_config) => {
                setConfig(def_config);
            });
            console.error(e);
        }

        if (isFirefoxLike)
            browser.storage.local
                .get(["config"])
                .then(handleStorageConfig)
                .catch(handleStorageConfigError);
        else
            chrome.storage.local
                .get(["config"])
                .then(handleStorageConfig)
                .catch(handleStorageConfigError);
    }, []);

    const saveConfig = () => {
        const config: DownloadConfig = {
            download_threads_max: maxTh,
            download_threads_min: minTh,
            download: folder,
            total_threads: total_threads,
        };

        if (isFirefoxLike) {
            browser.runtime
                .sendMessage({ event: "SET_CONFIG", config })
                .then((v) => {
                    console.log("Config saved.", v);
                });
        } else {
            chrome.runtime
                .sendMessage({ event: "SET_CONFIG", config })
                .then((v) => {
                    console.log("Config saved.", v);
                });
        }

        if (isFirefoxLike) {
            browser.storage.local.set({ config });
        } else {
            chrome.storage.local.set({ config });
        }
    };

    return (
        <div className="sidebar_app">
            <img className="sidebar_logo" src={logo} color="white" />
            <h1 className="sidebar_title">Download Manager</h1>

            <div className="form-group">
                <label htmlFor="max_threads">
                    Maximum number of download threads:
                </label>
                <input
                    id="max_threads"
                    type="number"
                    value={maxTh}
                    min={1}
                    max={total_threads}
                    onChange={(e) => setMaxTh(parseInt(e.target.value))}
                />
            </div>
            <div className="form-group">
                <label htmlFor="min_threads">
                    Minimum number of download threads:
                </label>
                <input
                    id="min_threads"
                    type="number"
                    value={minTh}
                    min={1}
                    max={total_threads}
                    onChange={(e) => setMinTh(parseInt(e.target.value))}
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

            {statusLabel()}

            <p className="sidebar_description">
                Settings are sent to the service worker, which manages the
                WebSocket connection.{" "}
                <a
                    href="https://github.com/KarboXXX/rust-download-manager"
                    target="_blank"
                    rel="noopener noreferrer">
                    README
                </a>
                .
            </p>
            <p className="sidebar_description">
                If settings aren't loading, try opening the service app with
                '--monitor' and refreshing this page
            </p>
        </div>
    );
};

export default SidebarApp;
