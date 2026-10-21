package main

import (
	"net/http"
	"net/http/httptest"
	"testing"
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
