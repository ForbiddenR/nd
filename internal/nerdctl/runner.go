package nerdctl

import (
	"errors"
	"fmt"
	"os"
	"os/exec"
)

// Runner executes nerdctl commands
type Runner struct{}

// NewRunner creates a new Runner
func NewRunner() *Runner {
	return &Runner{}
}

// Command represents a nerdctl compose command
type Command string

const (
	CommandUp      Command = "up"
	CommandDown    Command = "down"
	CommandEnter   Command = "enter"
	CommandLogs    Command = "logs"
	CommandRestart Command = "restart"
)

// ErrServiceRequired is returned when a command requires a service but none was provided
var ErrServiceRequired = errors.New("service name is required")

// Run executes a nerdctl compose command
func (r *Runner) Run(cmd Command, service string) error {
	switch cmd {
	case CommandUp:
		return r.runCompose("up", "-d")
	case CommandDown:
		return r.runCompose("down")
	case CommandEnter:
		if service == "" {
			return ErrServiceRequired
		}
		return r.execInteractive(service)
	case CommandLogs:
		if service == "" {
			return ErrServiceRequired
		}
		return r.logs(service)
	case CommandRestart:
		if service == "" {
			return ErrServiceRequired
		}
		return r.runCompose("restart", service)
	default:
		return fmt.Errorf("unknown command: %s", cmd)
	}
}

// runCompose runs a nerdctl compose command
func (r *Runner) runCompose(args ...string) error {
	cmd := exec.Command("nerdctl", append([]string{"compose"}, args...)...)
	cmd.Stdout = os.Stdout
	cmd.Stderr = os.Stderr
	cmd.Stdin = os.Stdin

	if err := cmd.Run(); err != nil {
		return fmt.Errorf("nerdctl compose %v: %w", args, err)
	}
	return nil
}

// execInteractive runs nerdctl compose exec with an interactive shell
func (r *Runner) execInteractive(service string) error {
	// Try bash first, then fallback to sh
	shells := []string{"bash", "sh"}
	var lastErr error

	for _, shell := range shells {
		cmd := exec.Command("nerdctl", "compose", "exec", "-it", service, shell)
		cmd.Stdout = os.Stdout
		cmd.Stderr = os.Stderr
		cmd.Stdin = os.Stdin

		if err := cmd.Run(); err == nil {
			return nil
		} else {
			lastErr = err
		}
	}

	return fmt.Errorf("failed to start shell in container %s: %w", service, lastErr)
}

// logs shows logs for a service
func (r *Runner) logs(service string) error {
	cmd := exec.Command("nerdctl", "compose", "logs", "-f", service)
	cmd.Stdout = os.Stdout
	cmd.Stderr = os.Stderr
	cmd.Stdin = os.Stdin

	if err := cmd.Run(); err != nil {
		return fmt.Errorf("failed to get logs for service %s: %w", service, err)
	}
	return nil
}

// CheckNerdctl checks if nerdctl is available
func (r *Runner) CheckNerdctl() error {
	cmd := exec.Command("nerdctl", "version")
	if err := cmd.Run(); err != nil {
		return fmt.Errorf("nerdctl not found: %w", err)
	}
	return nil
}
