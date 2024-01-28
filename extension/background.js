let socket;

if (typeof browser !== 'undefined') {
    function try_websocket_connection() {
        if (!socket || socket.readyState != WebSocket.OPEN) {
            socket = new WebSocket("ws://127.0.0.1:6969");
            socket.onopen = function(event) {
                console.log("websocket connection opened");
                socket.send("Web Monitoring Connected.");

                browser.downloads.onCreated.addListener(async (downloaded_item) => {
                    // console.log(downloaded_item.url);
                    let formatted = downloaded_item.url + "{|}" + downloaded_item.filename;
                    socket.send(formatted);
                    console.log('sending socket:', formatted);
                    browser.downloads.cancel(downloaded_item.id);
                    browser.downloads.erase({id: downloaded_item.id});
                    
                });
                
                // browser.webRequest.onBeforeRequest.addListener(
                //     async (details) => {
                //         if (details.method === "GET" && details.url.startsWith("http")) {
                //             socket.send(details.url);
                //         }
                //     },
                //     { urls: ["<all_urls>"] },
                //     ["blocking"]
                // );
            };

            socket.onerror = function(event) {
                console.error("Websocket error:", event);
                return;
            };

            socket.onclose = function(event) {
                console.info("Websocket connection closed.");
                socket = undefined;
                return;
            }
        }
    }

    if (!socket || socket.readyState === WebSocket.CLOSED) setInterval(try_websocket_connection, 20 * 1000) // every 20 seconds.
    console.log('Extension is running in Firefox');
} else if (typeof chrome !== 'undefined') {
    
    console.log('Extension is running in Chromium-based browser');
} else {
  console.log('Extension is running in an unsupported environment');
}

