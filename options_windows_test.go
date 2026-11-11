package main

import (
	"errors"
	"syscall"
	"testing"

	"fyne.io/fyne/v2"
	"fyne.io/fyne/v2/driver"
)

type nativeTestWindow struct {
	fyne.Window
	context any
}

func (w nativeTestWindow) RunNative(fn func(any)) {
	fn(w.context)
}

func TestSetWindowPinned(t *testing.T) {
	original := setWindowPos
	defer func() { setWindowPos = original }()
	var args []uintptr
	setWindowPos = func(values ...uintptr) (uintptr, uintptr, error) {
		args = values
		return 1, 0, syscall.Errno(0)
	}
	w := nativeTestWindow{context: driver.WindowsWindowContext{HWND: 123}}
	for _, tt := range []struct {
		pinned bool
		after  uintptr
	}{{true, ^uintptr(0)}, {false, ^uintptr(1)}} {
		if err := setWindowPinned(w, tt.pinned); err != nil {
			t.Fatal(err)
		}
		if len(args) != 7 || args[0] != 123 || args[1] != tt.after || args[6] != 0x13 {
			t.Fatalf("unexpected SetWindowPos arguments: %v", args)
		}
	}
	setWindowPos = func(...uintptr) (uintptr, uintptr, error) {
		return 0, 0, syscall.ERROR_ACCESS_DENIED
	}
	if err := setWindowPinned(w, true); !errors.Is(err, syscall.ERROR_ACCESS_DENIED) {
		t.Fatalf("expected SetWindowPos error, got %v", err)
	}
	if err := setWindowPinned(nativeTestWindow{context: driver.WindowsWindowContext{}}, true); err == nil {
		t.Fatal("expected an error for an unavailable window")
	}
}
