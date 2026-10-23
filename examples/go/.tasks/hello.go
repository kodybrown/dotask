// dotask: 1
// description: Greet someone using the bundled Go SDK and a declared local package.
// options: [{name: name, alias: n, default: Developer}]
// requires: [{kind: file, value: _support/message/message.go}]
// end-dotask
package main

import (
	dotask "dotask.local/sdk"
	"dotask.local/task/_support/message"
	"fmt"
)

func main() { dotask.Run(task) }
func task(c *dotask.BuildContext) error {
	name, err := c.String("name")
	if err != nil {
		return err
	}
	greeting, err := dotask.SettingAs[string](c, "greeting")
	if err != nil {
		return err
	}
	fmt.Println(message.Greet(greeting, name))
	return nil
}
