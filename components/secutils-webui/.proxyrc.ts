import type { Server } from 'connect';
import { createProxyMiddleware } from 'http-proxy-middleware';

module.exports = function (app: Server) {
  app.use('/api', createProxyMiddleware({ target: 'http://127.0.0.1:7070/api' }));
  app.use('/docs', createProxyMiddleware({ target: 'http://127.0.0.1:7373/docs' }));
  app.use('/self-service', createProxyMiddleware({ target: 'http://127.0.0.1:4433/self-service' }));

  // Route *.webhooks.localhost requests to the API server.
  // Mirrors production Traefik: HostRegexp + replacePath:/api/webhooks
  // The browser resolves *.localhost to 127.0.0.1 (Chrome/Firefox).
  app.use((req, res, next) => {
    const host = req.headers.host || '';
    if (host.includes('.webhooks.')) {
      const originalPath = req.originalUrl || req.url;
      return createProxyMiddleware({
        target: 'http://127.0.0.1:7070',
        pathRewrite: () => '/api/webhooks',
        headers: {
          'X-Forwarded-Host': host.split(':')[0],
          ...(originalPath ? { 'X-Replaced-Path': originalPath } : {}),
        },
      })(req, res, next);
    }
    next();
  });
};
