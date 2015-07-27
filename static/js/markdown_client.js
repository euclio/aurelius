document.addEventListener('DOMContentLoaded', function() {
    var previewWindow = document.getElementById('markdown-preview');
    var webSocketUrl = ('ws://localhost:' +
                        previewWindow.getAttribute('data-websocket-port'));

    var socket = new ReconnectingWebSocket(webSocketUrl);
    socket.maxReconnectInterval = 5000;

    socket.onopen = function(event) {
        console.log("Connection made");
    }

    socket.onmessage = function(event) {
        console.log('Data received: ' + event.data);
        document.getElementById('markdown-preview').innerHTML = event.data;
    }

    socket.onerror = function(event) {
        console.log('error connecting: ' + event.data)
    }

    socket.onclose = function(event) {
        // Close the browser window.
        window.open('', '_self', '');
        window.close();
    }
});
