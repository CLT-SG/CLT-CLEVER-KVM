# TLS/WSS Configuration Guide

## Overview

CLT-CLEVER-KVM supports secure WebSocket connections (WSS) for NoVNC and audio streaming. This guide explains how to set up TLS termination for production deployments.

## Why TLS is Required

Modern browsers require secure contexts (HTTPS/WSS) for:
- **NoVNC**: Browser-based VNC clients won't work over plain HTTP in secure contexts
- **WebRTC**: Many browsers require HTTPS for WebRTC features
- **Security**: Encrypted communication prevents eavesdropping and man-in-the-middle attacks
- **Compliance**: Many environments require encrypted communications

## Architecture

CLT-CLEVER-KVM follows the industry-standard approach of **TLS termination at a reverse proxy**:

```
Browser (HTTPS/WSS)
    ↓
Reverse Proxy (TLS Termination)
    ↓ 
CLT-CLEVER-KVM (Plain WebSocket)
```

### Benefits of This Approach

1. **Performance**: Reverse proxies are highly optimized for TLS
2. **Certificate Management**: Centralized at proxy level
3. **Flexibility**: Easy to update TLS configurations
4. **Standard Practice**: Used by AWS, Cloudflare, Google Cloud, etc.
5. **Separation of Concerns**: Application handles business logic, proxy handles TLS

## Configuration Steps

### Step 1: Enable TLS URLs in CLT-CLEVER-KVM

From the frontend or via Tauri command:

```typescript
// Enable TLS URL generation
await invoke('set_use_tls_urls', { useTls: true });
```

This changes URL generation from:
- `ws://hostname:6900/audio` → `wss://hostname:6900/audio`
- `ws://hostname:5900/websockify` → `wss://hostname:5900/websockify`

### Step 2: Set Up Reverse Proxy

Choose one of the following options:

## Option 1: Nginx (Recommended)

### Installation

```bash
# Ubuntu/Debian
sudo apt update
sudo apt install nginx

# CentOS/RHEL
sudo yum install nginx

# macOS
brew install nginx
```

### Configuration

Create `/etc/nginx/sites-available/clever-kvm`:

```nginx
# HTTP to HTTPS redirect
server {
    listen 80;
    server_name your-domain.com;
    return 301 https://$server_name$request_uri;
}

# HTTPS server
server {
    listen 443 ssl http2;
    server_name your-domain.com;
    
    # SSL Certificate (use Let's Encrypt or your own certificate)
    ssl_certificate /etc/ssl/certs/your-domain.com.crt;
    ssl_certificate_key /etc/ssl/private/your-domain.com.key;
    
    # SSL Configuration (Mozilla Modern Configuration)
    ssl_protocols TLSv1.3 TLSv1.2;
    ssl_ciphers 'ECDHE-ECDSA-AES128-GCM-SHA256:ECDHE-RSA-AES128-GCM-SHA256:ECDHE-ECDSA-AES256-GCM-SHA384:ECDHE-RSA-AES256-GCM-SHA384';
    ssl_prefer_server_ciphers off;
    ssl_session_cache shared:SSL:10m;
    ssl_session_timeout 10m;
    
    # WebSocket Audio Stream
    location /audio {
        proxy_pass http://localhost:6900;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        
        # WebSocket specific
        proxy_read_timeout 86400;
        proxy_send_timeout 86400;
    }
    
    # VNC WebSocket (websockify)
    location /websockify {
        proxy_pass http://localhost:5900;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        
        # WebSocket specific
        proxy_read_timeout 86400;
        proxy_send_timeout 86400;
    }
    
    # Multiple monitors support
    location ~ ^/websockify/monitor-(\d+)$ {
        set $monitor_port 5900;
        # Calculate port: 5900 + monitor number
        # This is a simplified example - adjust based on your setup
        proxy_pass http://localhost:$monitor_port;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_read_timeout 86400;
        proxy_send_timeout 86400;
    }
}
```

### Enable and Start

```bash
# Enable site
sudo ln -s /etc/nginx/sites-available/clever-kvm /etc/nginx/sites-enabled/

# Test configuration
sudo nginx -t

# Reload Nginx
sudo systemctl reload nginx
```

## Option 2: Caddy (Automatic HTTPS)

Caddy automatically obtains and renews Let's Encrypt certificates!

### Installation

```bash
# Ubuntu/Debian
sudo apt install -y debian-keyring debian-archive-keyring apt-transport-https
curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/gpg.key' | sudo gpg --dearmor -o /usr/share/keyrings/caddy-stable-archive-keyring.gpg
curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/debian.deb.txt' | sudo tee /etc/apt/sources.list.d/caddy-stable.list
sudo apt update
sudo apt install caddy

# macOS
brew install caddy
```

### Configuration

Create `/etc/caddy/Caddyfile`:

```caddy
your-domain.com {
    # Audio WebSocket
    reverse_proxy /audio localhost:6900 {
        header_up Host {host}
        header_up X-Real-IP {remote}
        header_up X-Forwarded-For {remote}
        header_up X-Forwarded-Proto {scheme}
    }
    
    # VNC WebSocket
    reverse_proxy /websockify localhost:5900 {
        header_up Host {host}
        header_up X-Real-IP {remote}
        header_up X-Forwarded-For {remote}
        header_up X-Forwarded-Proto {scheme}
    }
}
```

### Start Caddy

```bash
sudo systemctl enable caddy
sudo systemctl start caddy
```

That's it! Caddy automatically obtains and renews SSL certificates from Let's Encrypt.

## Option 3: HAProxy

### Installation

```bash
# Ubuntu/Debian
sudo apt install haproxy

# CentOS/RHEL
sudo yum install haproxy
```

### Configuration

Edit `/etc/haproxy/haproxy.cfg`:

```haproxy
global
    maxconn 4096
    tune.ssl.default-dh-param 2048

defaults
    mode http
    timeout connect 5000ms
    timeout client 50000ms
    timeout server 50000ms

frontend wss_frontend
    bind *:443 ssl crt /etc/ssl/private/your-domain.com.pem
    default_backend audio_backend
    acl is_websockify path_beg /websockify
    use_backend websockify_backend if is_websockify

backend audio_backend
    server audio1 localhost:6900 check

backend websockify_backend
    server vnc1 localhost:5900 check
```

### Start HAProxy

```bash
sudo systemctl enable haproxy
sudo systemctl start haproxy
```

## SSL Certificate Options

### Option 1: Let's Encrypt (Free, Recommended)

#### Using Certbot with Nginx

```bash
# Install Certbot
sudo apt install certbot python3-certbot-nginx

# Obtain certificate
sudo certbot --nginx -d your-domain.com

# Auto-renewal is set up automatically
sudo certbot renew --dry-run
```

#### Using Caddy (Automatic)

Caddy handles everything automatically - just use your domain in the Caddyfile!

### Option 2: Self-Signed Certificate (Development/Testing)

**Note**: Self-signed certificates will show browser warnings. Use only for development.

```bash
# Generate self-signed certificate
openssl req -x509 -nodes -days 365 -newkey rsa:2048 \
    -keyout /etc/ssl/private/selfsigned.key \
    -out /etc/ssl/certs/selfsigned.crt \
    -subj "/C=SG/ST=Singapore/L=Singapore/O=CLT/CN=localhost"

# For Nginx, use these paths in ssl_certificate directives
# For Caddy, use: tls /etc/ssl/certs/selfsigned.crt /etc/ssl/private/selfsigned.key
```

### Option 3: Commercial Certificate

Purchase from a Certificate Authority (DigiCert, GlobalSign, etc.) and install according to their instructions.

## Trusting Self-Signed Certificates (Development)

If using self-signed certificates, you must trust them in your browser:

### Chrome/Edge

1. Navigate to `chrome://settings/certificates`
2. Go to "Authorities" tab
3. Click "Import"
4. Select your `.crt` file
5. Check "Trust this certificate for identifying websites"

### Firefox

1. Navigate to `about:preferences#privacy`
2. Scroll to "Certificates"
3. Click "View Certificates"
4. Go to "Authorities" tab
5. Click "Import"
6. Select your `.crt` file

### macOS

```bash
sudo security add-trusted-cert -d -r trustRoot -k /Library/Keychains/System.keychain /path/to/cert.crt
```

### Linux

```bash
sudo cp /path/to/cert.crt /usr/local/share/ca-certificates/
sudo update-ca-certificates
```

## Testing Your Setup

### Test WebSocket Connection

```bash
# Install wscat
npm install -g wscat

# Test audio WebSocket
wscat -c wss://your-domain.com/audio

# Test VNC WebSocket
wscat -c wss://your-domain.com/websockify
```

### Test with Browser

```javascript
// Open browser console and run:
const ws = new WebSocket('wss://your-domain.com/audio');
ws.onopen = () => console.log('✅ Connected');
ws.onerror = (e) => console.error('❌ Error:', e);
```

## NoVNC Integration

Once TLS is configured, connect NoVNC to your secure WebSocket:

```html
<!DOCTYPE html>
<html>
<head>
    <title>NoVNC</title>
    <script src="https://cdn.jsdelivr.net/npm/novnc/core/rfb.js"></script>
</head>
<body>
    <div id="screen"></div>
    <script>
        const rfb = new RFB(
            document.getElementById('screen'),
            'wss://your-domain.com/websockify',
            { credentials: { password: '' } }
        );
        
        rfb.addEventListener('connect', () => console.log('✅ VNC Connected'));
        rfb.addEventListener('disconnect', () => console.log('❌ VNC Disconnected'));
    </script>
</body>
</html>
```

## Troubleshooting

### WebSocket Connection Fails

1. **Check reverse proxy logs**:
   ```bash
   # Nginx
   sudo tail -f /var/log/nginx/error.log
   
   # Caddy
   sudo journalctl -u caddy -f
   
   # HAProxy
   sudo tail -f /var/log/haproxy.log
   ```

2. **Verify CLT-CLEVER-KVM is running**:
   ```bash
   # Check if ports are listening
   sudo netstat -tlnp | grep -E '5900|6900'
   ```

3. **Test without TLS**:
   ```bash
   wscat -c ws://localhost:6900/audio
   ```

### Certificate Issues

1. **Check certificate validity**:
   ```bash
   openssl s_client -connect your-domain.com:443 -servername your-domain.com
   ```

2. **Verify certificate chain**:
   ```bash
   openssl verify -CAfile /path/to/ca-bundle.crt /path/to/your-cert.crt
   ```

### Browser Shows "Not Secure"

- Verify certificate is valid and not expired
- Check that certificate matches the domain
- Ensure intermediate certificates are included
- Clear browser cache and SSL state

## Production Best Practices

1. **Use Let's Encrypt** for automatic certificate management
2. **Enable HTTP/2** for better performance
3. **Set up monitoring** for certificate expiration
4. **Configure rate limiting** to prevent abuse
5. **Enable logging** for debugging
6. **Use strong TLS ciphers** (TLSv1.3 recommended)
7. **Set up automatic backups** of certificates
8. **Document your configuration** for your team

## Security Considerations

1. **Keep certificates private**: Never commit private keys to version control
2. **Use strong passwords**: If password-protecting certificates
3. **Regular updates**: Keep reverse proxy software updated
4. **Monitor access**: Set up logging and alerts
5. **Rate limiting**: Prevent DoS attacks
6. **Firewall rules**: Only expose necessary ports

## Performance Tuning

### Nginx Tuning

```nginx
# Increase worker connections
events {
    worker_connections 4096;
}

# Enable gzip compression (not for WebSocket data)
gzip off;  # WebSocket traffic shouldn't be compressed

# Optimize SSL session cache
ssl_session_cache shared:SSL:50m;
ssl_session_timeout 1d;
```

### Caddy Tuning

Caddy is pre-tuned for most use cases. For high-traffic scenarios:

```caddy
{
    # Global options
    max_conns_per_ip 100
}
```

## Support

For issues specific to:
- **CLT-CLEVER-KVM**: Check the main documentation
- **Nginx**: https://nginx.org/en/docs/
- **Caddy**: https://caddyserver.com/docs/
- **HAProxy**: https://www.haproxy.org/documentation/
- **Let's Encrypt**: https://letsencrypt.org/docs/

## Summary

With this setup:
- ✅ NoVNC works in secure browser contexts
- ✅ All WebSocket traffic is encrypted
- ✅ Certificates are automatically managed (with Let's Encrypt)
- ✅ Industry-standard architecture
- ✅ Easy to scale and maintain
