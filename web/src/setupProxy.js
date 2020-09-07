const { createProxyMiddleware } = require("http-proxy-middleware");
module.exports = function(app) {
  app.use(
    "/sse",
    createProxyMiddleware({
      target: "http://localhost:8000/sse",
      changeHost: true
    })
  );
};
