document.addEventListener('DOMContentLoaded', function() {
    function syntaxHighlight() {
        if (hljs !== undefined) {
            var codeBlocks = document.querySelectorAll('pre code');
            for (var i = 0; i < codeBlocks.length; i++) {
                hljs.highlightBlock(codeBlocks[i]);
            }
        }
    }

    syntaxHighlight();
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
        syntaxHighlight();
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
