(function() {var implementors = {};
implementors['openssl'] = [];implementors['hyper'] = [];implementors['websocket'] = [];implementors['nickel'] = [];

            if (window.register_implementors) {
                window.register_implementors(implementors);
            } else {
                window.pending_implementors = implementors;
            }
        
})()