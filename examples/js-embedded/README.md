# Embedded JavaScript host

`main.ts` shows the injection boundary: the host owns provider construction and
passes a provider map to `mountApplication`. The application bundle is data-only
JSON. No credentials, cookies, remote assets, or checkout-specific client is
part of this example.
