package main

import (
	"fyne.io/fyne/v2"
	"fyne.io/fyne/v2/container"
	"fyne.io/fyne/v2/dialog"
	"fyne.io/fyne/v2/theme"
	"fyne.io/fyne/v2/widget"
)

var pinIcon = theme.NewThemedResource(fyne.NewStaticResource("pin.svg", []byte(
	`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path fill="#000000" d="M8 3h8v2l-1 1v5l3 3v2h-5v5l-1 2-1-2v-5H6v-2l3-3V6L8 5z"/></svg>`)))

func newPinButton(w fyne.Window, setPinned func(bool) error) *pointerButton {
	pinned := false
	var button *pointerButton
	button = newPointerButton("Pin", pinIcon, func() {
		if err := setPinned(!pinned); err != nil {
			showError(err, w)
			return
		}
		pinned = !pinned
		if pinned {
			button.Importance = widget.HighImportance
			button.SetText("Unpin")
		} else {
			button.Importance = widget.MediumImportance
			button.SetText("Pin")
		}
	})
	return button
}

func newOptionsButton(w fyne.Window, db *memoryDB, startupEnabled func() (bool, error), setStartup func(bool) error) *pointerButton {
	return newPointerButton("Settings", theme.SettingsIcon(), func() {
		themeSelect := widget.NewSelect(appThemes, nil)
		themeSelect.SetSelected(db.appTheme)
		var changeTheme func(string)
		changeTheme = func(value string) {
			previous := db.appTheme
			if err := db.setAppTheme(value); err != nil {
				themeSelect.OnChanged = nil
				themeSelect.SetSelected(previous)
				themeSelect.OnChanged = changeTheme
				showError(err, w)
				return
			}
			fyne.CurrentApp().Settings().SetTheme(pastelTheme{name: value})
		}
		themeSelect.OnChanged = changeTheme
		content := container.NewVBox(widget.NewLabel("App theme"), themeSelect)
		var startupErr error
		if startupEnabled != nil {
			var startup bool
			startup, startupErr = startupEnabled()
			startupCheck := newPointerCheck()
			startupCheck.Text = "Start Scrivi when I sign in"
			startupCheck.SetChecked(startup)
			if startupErr != nil {
				startupCheck.Disable()
			}
			var changeStartup func(bool)
			changeStartup = func(value bool) {
				if err := setStartup(value); err != nil {
					startupCheck.OnChanged = nil
					startupCheck.SetChecked(startup)
					startupCheck.OnChanged = changeStartup
					showError(err, w)
					return
				}
				startup = value
			}
			startupCheck.OnChanged = changeStartup
			content.Add(startupCheck)
		}
		d := dialog.NewCustomWithoutButtons("Settings", content, w)
		d.SetButtons([]fyne.CanvasObject{newPointerButton("Close", nil, d.Hide)})
		d.Show()
		if startupErr != nil {
			showError(startupErr, w)
		}
	})
}
