package main

import (
	"context"
	"crypto/sha256"
	"fmt"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"
)

func TestNewerVersion(t *testing.T) {
	for _, tc := range []struct {
		current string
		latest  string
		want    bool
	}{
		{"v1.0.0", "v1.0.1", true},
		{"v1.9.9", "v1.10.0", true},
		{"v1.9.9", "v2.0.0", true},
		{"v1.1.0", "v1.1.0", false},
		{"v2.0.0", "v1.99.0", false},
	} {
		t.Run(tc.current+"-"+tc.latest, func(t *testing.T) {
			got, err := newerVersion(tc.current, tc.latest)
			if err != nil || got != tc.want {
				t.Fatalf("got %v, %v; want %v", got, err, tc.want)
			}
		})
	}
	for _, invalid := range []string{"dev", "v1.0", "v1.0.1-beta", "v1.01.0", "v65536.0.0"} {
		if _, err := newerVersion("v1.0.0", invalid); err == nil {
			t.Errorf("accepted invalid version %q", invalid)
		}
	}
}

func TestFindUpdate(t *testing.T) {
	for _, tc := range []struct {
		name    string
		status  int
		body    string
		wantURL bool
		wantErr bool
	}{
		{"new release", 200, `{"tag_name":"v1.1.0","assets":[{"name":"scrivi-v1.1.0-windows-x64-setup.exe","browser_download_url":"https://untrusted.example/setup.exe"}]}`, true, false},
		{"same version", 200, `{"tag_name":"v1.0.0"}`, false, false},
		{"older release", 200, `{"tag_name":"v0.9.0"}`, false, false},
		{"prerelease", 200, `{"tag_name":"v2.0.0-beta","prerelease":true}`, false, false},
		{"draft", 200, `{"tag_name":"v2.0.0","draft":true}`, false, false},
		{"missing installer", 200, `{"tag_name":"v1.1.0","assets":[]}`, false, true},
		{"invalid version", 200, `{"tag_name":"../bad"}`, false, true},
		{"invalid response", 200, `{`, false, true},
		{"no releases", 404, ``, false, false},
		{"rate limited", 403, ``, false, true},
	} {
		t.Run(tc.name, func(t *testing.T) {
			server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
				if r.Header.Get("User-Agent") != "scrivi/v1.0.0" {
					t.Error("missing versioned user agent")
				}
				w.WriteHeader(tc.status)
				_, _ = w.Write([]byte(tc.body))
			}))
			defer server.Close()
			version, download, err := findUpdate(server.Client(), server.URL, "vespoli95/scrivi", "v1.0.0")
			if (err != nil) != tc.wantErr {
				t.Fatalf("unexpected error: %v", err)
			}
			if (download != "") != tc.wantURL {
				t.Fatalf("unexpected download URL: %q", download)
			}
			if tc.wantURL {
				want := "https://github.com/vespoli95/scrivi/releases/download/v1.1.0/scrivi-v1.1.0-windows-x64-setup.exe"
				if download != want || version != "v1.1.0" {
					t.Fatalf("unexpected update: %q %q", version, download)
				}
			}
		})
	}
}

func TestDownloadInstaller(t *testing.T) {
	const installer = "test installer contents"
	validChecksum := fmt.Sprintf("%x\n", sha256.Sum256([]byte(installer)))
	for _, tc := range []struct {
		name            string
		checksum        string
		checksumStatus  int
		installerStatus int
		truncated       bool
		oversized       bool
		wantErr         bool
	}{
		{name: "verified", checksum: validChecksum},
		{name: "mismatch", checksum: strings.Repeat("0", 64), wantErr: true},
		{name: "invalid checksum", checksum: "not a checksum", wantErr: true},
		{name: "short checksum", checksum: "abcd", wantErr: true},
		{name: "missing checksum", checksumStatus: http.StatusNotFound, wantErr: true},
		{name: "download failure", checksum: validChecksum, installerStatus: http.StatusServiceUnavailable, wantErr: true},
		{name: "interrupted download", checksum: validChecksum, truncated: true, wantErr: true},
		{name: "oversized download", checksum: validChecksum, oversized: true, wantErr: true},
	} {
		t.Run(tc.name, func(t *testing.T) {
			server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
				if r.URL.Path == "/setup.exe.sha256" {
					if tc.checksumStatus != 0 {
						w.WriteHeader(tc.checksumStatus)
					}
					fmt.Fprint(w, tc.checksum)
					return
				}
				if tc.installerStatus != 0 {
					w.WriteHeader(tc.installerStatus)
				}
				if tc.truncated {
					w.Header().Set("Content-Length", "100")
				}
				if tc.oversized {
					w.Header().Set("Content-Length", fmt.Sprint((128<<20)+1))
				}
				fmt.Fprint(w, installer)
			}))
			defer server.Close()
			dir := t.TempDir()
			path := filepath.Join(dir, "setup.exe")
			if err := os.WriteFile(path, []byte("previous download"), 0600); err != nil {
				t.Fatal(err)
			}
			got, err := downloadInstaller(context.Background(), server.Client(), server.URL+"/setup.exe", dir)
			if (err != nil) != tc.wantErr {
				t.Fatalf("unexpected error: %v", err)
			}
			want := installer
			if tc.wantErr {
				want = "previous download"
				if got != "" {
					t.Fatalf("failed download returned installer path %q", got)
				}
			} else if got != path {
				t.Fatalf("got installer path %q, want %q", got, path)
			}
			data, err := os.ReadFile(path)
			if err != nil || string(data) != want {
				t.Fatalf("installer contents %q, error %v; want %q", data, err, want)
			}
			files, err := os.ReadDir(dir)
			if err != nil || len(files) != 1 {
				t.Fatalf("temporary downloads were not cleaned up: %v, %v", files, err)
			}
		})
	}
}

func TestDownloadInstallerCanceled(t *testing.T) {
	ctx, cancel := context.WithCancel(context.Background())
	cancel()
	if path, err := downloadInstaller(ctx, &http.Client{}, "https://github.com/setup.exe", t.TempDir()); err == nil || path != "" {
		t.Fatalf("canceled download returned %q, %v", path, err)
	}
}

func TestWatchUpdates(t *testing.T) {
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()
	checks := make(chan struct{})
	stopped := make(chan struct{})
	go func() {
		watchUpdates(ctx, time.Millisecond, func() {
			select {
			case checks <- struct{}{}:
			case <-ctx.Done():
			}
		})
		close(stopped)
	}()
	for range 2 {
		select {
		case <-checks:
		case <-time.After(5 * time.Second):
			t.Fatal("update watcher did not check at startup and again while running")
		}
	}
	cancel()
	select {
	case <-stopped:
	case <-time.After(5 * time.Second):
		t.Fatal("update watcher did not stop when the app closed")
	}

	watchUpdates(ctx, time.Millisecond, func() {
		t.Error("canceled watcher must not check for updates")
	})
}
