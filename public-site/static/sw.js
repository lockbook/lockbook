self.skipWaiting();

self.addEventListener('activate', function (event) {
  event.waitUntil(
    self.clients.matchAll({ type: 'window' }).then(function (clients) {
      return Promise.all(clients.map(function (client) { return client.navigate(client.url); }));
    })
  );
});
