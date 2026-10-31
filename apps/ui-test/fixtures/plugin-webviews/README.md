# Disposable WebView fixtures

`loopback.crt` and `loopback.key` are public test-only, self-signed TLS material,
generated offline with .NET CertificateRequest. They are not credentials,
production certificates, trust pins or a trusted publisher identity. Nothing
is installed in the Windows certificate store.

The harness binds an OS-selected literal IPv4 loopback port. Its Host positive
control accepts this certificate only for that exact owned endpoint. WebView2
certificate verification remains enabled. The independent native request filter
must return a refusal and increment its counter before the TLS canary receives
any request. HTTPS avoids a mixed-content rejection masking that native layer.
