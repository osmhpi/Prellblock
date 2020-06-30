# How to use ThingsBoard with Prellblock

This is a short description of how to use ThingsBoard in combination with Prellblock
to export written data to Thingsboard.

## 1. Running ThingsBoard

We provide a sample [docker-compose.yml](./docker-compose.yml) file for starting thingsboard:
```sh
$ docker-compose up
```

After executing this command you can open `http://{your-host-ip}:8080` in you browser (for ex. `http://localhost:8080`).
You should see ThingsBoard login page.
Use the following default credentials:

- System Administrator: sysadmin@thingsboard.org / sysadmin
- Tenant Administrator: tenant@thingsboard.org / tenant
- Customer User: customer@thingsboard.org / customer

Use the **tenant** account to create an manage devices.

## 2. Retrieving an tenant account's id

1. Use the `sysadmin` account for logging in:
To retreive the tenant's account ID, you first have to an access token for an admin's account.
You can do this by posting to the authentication url with an admin's account name and password.
This is the default admin account and should be changed:
```sh
curl -v -X POST -d "{\"username\": \"sysadmin@thingsboard.org\",\"password\":\"sysadmin\"}" http://localhost:8080/api/auth/login --header "Content-Type:application/json"
```
THE API will send a token to you which will be used in the next step.

2. Retrieve the tenant's ID:
Using the gained `token` you have access to API request like the following:
```sh
curl -X GET --header 'Accept: application/json' --header 'X-Authorization: Bearer:$ACCESS_TOKEN' 'https://host:port/api/tenants'
```
Which will retrieve all tenant accounts and their IDs.
The ID is necessary in the next step.

## 3. Tell Prellblock the tenant's ID

You can compile `prellblock` with the `thingsboard` feature. This will enable the communication with a thingsboard instance.

You'll need to set the following environment variables to connect to thingboard:
>"THINGSBOARD_USER_NAME"  
*(this is your (tenant) account's name)*

>"THINGSBOARD_PASSWORD"   
*(this ist your (tenant) account's password)*

>"THINGSBOARD_TENANT_ID"  
*(this ist your (tenant) account's id)*

After that, the setup should be automatic.
Prellblock retrieves an access token automatically and creates devices.
There won't happen anything, if you do not subscribe to a timeseries in the `../config/subscription_config.toml`.

## 4. Setup subscriptions

The subscriptions for ThingsBoard are defined in a TOML-file with the following content:
**TODO**

## 5. Sending tess values to ThingsBoard via command line

You can send value to a device using the following command (the `$ACCESS_TOKEN` from [step 2](#2-retrieving-a-tenants-id):
```sh
curl -v -X POST -d "{\"temperature\": 25}" http://localhost:8080/api/v1/$ACCESS_TOKEN/telemetry --header "Content-Type:application/json"
```
