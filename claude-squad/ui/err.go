package ui

import (
	"github.com/charmbracelet/lipgloss"
	"strings"
)

type ErrBox struct {
	height, width int
	err           error
}

var errStyle = lipgloss.NewStyle().Foreground(lipgloss.AdaptiveColor{
	Light: "#FF0000",
	Dark:  "#FF0000",
})

func NewErrBox() *ErrBox {
	return &ErrBox{}
}

func (e *ErrBox) SetError(err error) {
	e.err = err
}

func (e *ErrBox) Clear() {
	e.err = nil
}

func (e *ErrBox) SetSize(width, height int) {
	e.width = width
	e.height = height
}

func (e *ErrBox) String() string {
	var err string
	if e.err != nil {
		err = e.err.Error()
		lines := strings.Split(err, "\n")
		err = strings.Join(lines, "//")
		if len(err) > e.width-3 && e.width-3 >= 0 {
			err = err[:e.width-3] + "..."
		}
	}
	return lipgloss.Place(e.width, e.height, lipgloss.Center, lipgloss.Center, errStyle.Render(err))
}
